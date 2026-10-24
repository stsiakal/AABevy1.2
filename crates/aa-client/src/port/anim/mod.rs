//! Animation-graph port, steps 1-4 (partial): pure leaves, the static graph loader, the graph
//! state machine, the LMG capabilities/per-layer bodies and the unit-parent rider copy.
//!
//! Step 1 is a line-by-line port of the pure functions used by the 1.2 animation graph:
//! `CAnimationGraph::SStateInfo::EvaluateGuards` (`cryaction+0x5f7a80`), the integer/float/key
//! input encoders (`+0x604b10`, `+0x605180`, `+0x611470`, `+0x6115d0`), the locomotion FOURCC
//! table (`cryanimation+0x1568fc0`), and the three mount placement helpers in `x2game.dll`
//! (`+0x909ae90`, `+0x90a2c90`, `+0x909b1a0`).
//!
//! Step 2 is the static graph loader: the XML under `game/animations/graphs/` is parsed into
//! the same input/state/transition tables the original builds, with the original's encoding
//! (guard packing, float quantisation, key indexing, state ordering and parameter expansion).
//!
//! Every item names the decompiled function it comes from. None of this is wired into the
//! playable client yet.

#![allow(dead_code, unused_imports)]

mod ag;
mod fourcc;
mod graph;
mod guards;
mod inputs;
mod lmg;
mod machine;
mod rider;
mod side;

#[cfg(test)]
mod tests;

pub use ag::{ag_path, load_graph_ag, load_graph_ag_from_pak};
pub use fourcc::{FOURCC_TABLE, fourcc_case};
pub use graph::{
    AnimationGraph, GraphInput, GraphSource, InputKind, StateIndex, StateIndexEntry, StateInfo,
    TemplateInfo, TransitionInfo, load_graph, load_graph_from_pak,
};
pub use guards::evaluate_guards;
pub use inputs::{
    encode_float, encode_float_str, encode_int, encode_int_str, encode_key, encode_key_index,
    parse_float, parse_int,
};
pub use lmg::{fun_31558550, fun_3156bf40, fun_31574e70, AnimRow, AnimSet, Caps, Desc, Layer, Lmg};
pub use rider::{actor_unit_model_update, RiderInputs};
pub use machine::{GraphEvent, GraphState, InputValues, UpdateKind};
pub use side::{Placement, Vec3, approach_side, entry_side, side_placement};

/// `0x3999cffc` in `x2game.dll`: the 2.0 used by every quaternion/matrix conversion.
pub const TWO: f32 = 2.0;
/// `0x3999c488`: 1.0.
pub const ONE: f32 = 1.0;
/// `0x3999cc18`: `FLT_MIN` added to the squared length before `rsqrtss`.
pub const FLT_MIN: f32 = 1.175_494_4e-38;
/// `0x3999c5f0`: `-0.0`, used to flip a float by XOR.
pub const NEG_ZERO: f32 = -0.0;
/// `0x399a2d28` / `0x399a2d24`: the entry-side back/front dot thresholds.
pub const ENTRY_DOT: f32 = 0.96;
/// `0x39ac3878`: the 3 m send range squared.
pub const SEND_RANGE_SQ: f32 = 9.0;
/// `0x39ac3888` / `0x39ac388c`: the side-placement distance and unit scale.
pub const SIDE_DISTANCE: f32 = 1.5;
pub const SIDE_SCALE: f32 = 1.0;
