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

/// The values the two engine sync thunks (`thunk_FUN_3a98bb00` = `0x3a98bb00` and
/// `thunk_FUN_3a9918bb` = `0x3a9918bb`) leave in the movement object at
/// `+0x64..+0x8c`.  They run engine-side and are inputs to the wrapper.
#[derive(Clone, Copy, Debug)]
pub struct FrameSync {
    /// `+0x64/+0x68/+0x6c` (the movement velocity).
    pub velocity: [u32; 3],
    /// `+0x70..+0x7c`.
    pub q70: [u32; 4],
    /// `+0x80` (the ctor writes 1.0).
    pub one_80: u32,
    /// `+0x84`.
    pub flag_84: u8,
    /// `+0x88`.
    pub word_88: u32,
    /// `+0x8c`.
    pub flag_8c: u8,
}

impl Default for FrameSync {
    fn default() -> Self {
        Self {
            velocity: [0; 3],
            q70: [0; 4],
            one_80: 1.0f32.to_bits(),
            flag_84: 0,
            word_88: 0,
            flag_8c: 0,
        }
    }
}

/// The actor fields `FUN_39135fe0` (`CPlayer::ProcessFrameMovement`) writes at
/// `actor+0x308..0x330` after the two engine sync thunks.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameMovementActor {
    /// `actor+0x308/+0x30c/+0x310` <- mover `+0x64/+0x68/+0x6c`.
    pub field_308: [u32; 3],
    /// `actor+0x314/+0x318/+0x31c/+0x320/+0x324` <- mover `+0x70..+0x80`.
    pub field_314: [u32; 5],
    /// `actor+0x328` <- mover `+0x84`.
    pub field_328: u8,
    /// `actor+0x32c` <- mover `+0x88`.
    pub field_32c: u32,
    /// `actor+0x330` <- mover `+0x8c`.
    pub field_330: u8,
}

/// `FUN_39135fe0` (0x39135fe0): build `CPlayerMovement` (`0x3a6ab5f3`), run the
/// two sync thunks and copy the movement object fields `+0x64..+0x8c` into the
/// actor `+0x308..0x330` (offsets from the instruction sequence
/// `0x391360b6..0x39136126`).
pub fn process_frame_movement(
    input: &MoverInput,
    sync: &FrameSync,
) -> (CPlayerMovement, FrameMovementActor) {
    let mut mover = CPlayerMovement::process(input);
    mover.velocity = [
        f32::from_bits(sync.velocity[0]),
        f32::from_bits(sync.velocity[1]),
        f32::from_bits(sync.velocity[2]),
    ];
    mover.one_80 = f32::from_bits(sync.one_80);
    mover.flag_84 = sync.flag_84;
    mover.zero_88 = sync.word_88;
    mover.flag_8c = sync.flag_8c;
    let actor = FrameMovementActor {
        field_308: sync.velocity,
        field_314: [
            sync.q70[0],
            sync.q70[1],
            sync.q70[2],
            sync.q70[3],
            sync.one_80,
        ],
        field_328: sync.flag_84,
        field_32c: sync.word_88,
        field_330: sync.flag_8c,
    };
    (mover, actor)
}
