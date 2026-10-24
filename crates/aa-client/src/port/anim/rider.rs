//! Rider animation-graph copy, step 5 of the animation-graph port.
//!
//! `X2::ActorUnitModel::Update FUN_390dd080` calls `FUN_390dcc80` every frame. When the rider
//! is attached to a unit parent, `FUN_390dcc80` reads the parent graph's `PseudoSpeed`,
//! `Stance` and `RequestedTurnSpeedLZ` (`GetInputId` graph vtable `+0x60`, `GetInputFloat`
//! `+0x5c`, `GetInputString` `+0x24`) and writes the two floats into the rider graph
//! (`SetInput` vtable `+0xc`). A `swim` stance forces the copied `PseudoSpeed` to zero.
//!
//! The capture that gates this port sets the summoned horse's graph inputs and reads the
//! local player's graph inputs after the original frame update (`goldens/rider_copy.json`).

/// The unit-parent branch of `FUN_390dcc80`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiderInputs {
    pub pseudo_speed: f32,
    pub turn_speed: f32,
    pub swim: bool,
}

impl RiderInputs {
    /// The two floats written into the rider graph.
    pub fn copy(&self) -> (f32, f32) {
        let speed = if self.swim { 0.0 } else { self.pseudo_speed };
        (speed, self.turn_speed)
    }
}

/// `FUN_390dd080` (`X2::ActorUnitModel::Update`) applies the rider copy and then updates the
/// camera distance from the mount speed. Only the graph-input half is ported here.
pub fn actor_unit_model_update(inputs: RiderInputs) -> (f32, f32) {
    inputs.copy()
}
