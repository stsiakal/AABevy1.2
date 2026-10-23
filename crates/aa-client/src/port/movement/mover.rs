//! `CPlayerMovement` (0x3a6ab5f3) and the quadruped controller ctors
//! (`FUN_3912c640`, `FUN_3912e0c0`).
//!
//! The ctor copies actor movement state into a stack object; the tail call
//! `(**(entity vtable + 0x98))()` returns a world-position pointer which the
//! continuation stores at object `+0x50`.  The port takes that position as an
//! input (`entity_pos`), so no engine object is needed.

/// The 13-dword movement block copied from the input request.
pub const REQUEST_DWORDS: usize = 13;

#[derive(Clone, Copy, Debug, Default)]
pub struct MoverInput {
    pub dt: f32,
    pub request: [u32; REQUEST_DWORDS],
    /// Result of `(**(entity vtable + 0x98))()`: the actor's world position.
    pub entity_pos: [f32; 3],
    pub stride: f32,
    pub velocity: [f32; 3],
    pub movement_scalar: f32,
    pub flag_161: u8,
    pub swim_a: [f32; 2],
    pub swim_b: [f32; 2],
    pub val_9dc: f32,
    pub flag_79: u8,
    pub flag_78: u8,
    pub val_284: f32,
}

/// Value fields of the `CPlayerMovement` object (0xC0 bytes).  The pointer
/// fields are not compared by the goldens.
#[derive(Clone, Copy, Debug, Default)]
pub struct CPlayerMovement {
    pub dt: f32,
    pub request: [u32; REQUEST_DWORDS],
    pub entity_pos: [f32; 3],
    pub stride: f32,
    pub velocity: [f32; 3],
    pub movement_scalar: f32,
    pub one_80: f32,
    pub flag_84: u8,
    pub zero_88: u32,
    pub flag_8c: u8,
    pub vec_90: [f32; 3],
    pub flag_9c: u8,
    pub swim_a: [f32; 2],
    pub swim_b: [f32; 2],
    pub flag_b0: u8,
    pub val_b4: f32,
    pub flag_b8: u8,
    pub flag_b9: u8,
    pub flag_ba: u8,
    pub val_bc: f32,
}

impl CPlayerMovement {
    /// `FUN_3a6ab5f3` (reached from `0x3913a620`).
    pub fn process(input: &MoverInput) -> Self {
        Self {
            dt: input.dt,
            request: input.request,
            entity_pos: input.entity_pos,
            stride: input.stride,
            velocity: input.velocity,
            movement_scalar: input.movement_scalar,
            one_80: 1.0,
            flag_84: 0,
            zero_88: 0,
            flag_8c: 0,
            vec_90: [0.0; 3],
            flag_9c: input.flag_161,
            swim_a: input.swim_a,
            swim_b: input.swim_b,
            flag_b0: 0,
            val_b4: input.val_9dc,
            flag_b8: input.flag_79,
            flag_b9: input.flag_78,
            flag_ba: 0,
            val_bc: input.val_284,
        }
    }
}

/// `FUN_3912c640`: quadruped controller scope A.  `random` is the value
/// returned by `CryRandomFloat()` (0..1), recorded by the capture.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadrupedScopeA {
    pub dt: f32,
    pub request: [u32; REQUEST_DWORDS],
    pub entity_pos: [f32; 3],
    pub stride: f32,
    pub velocity: [f32; 3],
    pub movement_scalar: f32,
    pub flag_161: u8,
    pub val_918: f32,
    pub flag_78: u8,
    pub random: f32,
}

impl QuadrupedScopeA {
    /// `FUN_3912c640`.
    pub fn process(input: &QuadrupedScopeA) -> Self {
        Self {
            dt: input.dt,
            request: input.request,
            entity_pos: input.entity_pos,
            stride: input.stride,
            velocity: input.velocity,
            movement_scalar: input.movement_scalar,
            flag_161: input.flag_161,
            val_918: input.val_918,
            flag_78: input.flag_78,
            random: input.random,
        }
    }
}

/// `FUN_3912e0c0`: quadruped controller scope B (copies the actor quats and
/// the request direction; no entity call).
#[derive(Clone, Copy, Debug, Default)]
pub struct QuadrupedScopeB {
    pub dt: f32,
    pub quats: [[f32; 4]; 3],
    pub support: [[f32; 4]; 2],
    pub anchor: [f32; 3],
    pub request_dir: [f32; 3],
}

impl QuadrupedScopeB {
    /// `FUN_3912e0c0`.
    pub fn process(input: &QuadrupedScopeB) -> Self {
        *input
    }
}

/// The actor fields `FUN_39135fe0` (`CPlayer::ProcessFrameMovement`) writes at
/// `actor+0x308..0x330` after the two engine sync thunks
/// (`thunk_FUN_3a98bb00`, `thunk_FUN_3a9918bb`, U7).  The thunks themselves are
/// engine callbacks and are exposed as the two flags below; everything else is
/// the stack-object copy performed by the wrapper.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameMovementActor {
    /// `actor+0x308` and `+0x30c`: first copy (`local_84/local_80`).
    pub field_308: [u32; 3],
    /// `actor+0x314..0x324`.
    pub field_314: [u32; 5],
    /// `actor+0x328`.
    pub field_328: u8,
    /// `actor+0x32c`.
    pub field_32c: u32,
    /// `actor+0x330`.
    pub field_330: u8,
}

/// `FUN_39135fe0` wrapper result: the mover object, the two sync-thunk outputs
/// (recorded as bytes; U7) and the actor field copy.
pub fn process_frame_movement(
    input: &MoverInput,
    sync_a: [u8; 4],
    sync_b: [u8; 4],
) -> (CPlayerMovement, FrameMovementActor) {
    let mover = CPlayerMovement::process(input);
    let actor = FrameMovementActor {
        field_308: [
            mover.dt.to_bits(),
            mover.request[0],
            mover.request[1],
        ],
        field_314: [
            mover.request[2],
            mover.request[3],
            mover.request[4],
            mover.request[5],
            sync_a[0] as u32,
        ],
        field_328: sync_a[1],
        field_32c: sync_b[0] as u32,
        field_330: sync_b[1],
    };
    (mover, actor)
}
