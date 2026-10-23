//! The 0x9C-byte `MovementRequest` built by `FUN_39404d50` on the stack and submitted
//! through `GameClient vtable +0x3C` (`0x394063a6`).
//!
//! `FUN_390a8dc0` (0x390a8dc0, 245 bytes) is the constructor/reset: it fills the
//! sentinels reproduced by [`MovementRequest::default`].  The pointer fields `+0x64` and
//! `+0x80` are the module address `0x39ac3044` in the shipped 1.2 image; they are kept
//! as `u32` so the golden byte comparison is exact.

pub const REQUEST_SIZE: usize = 0x9C;
pub const DEFAULT_REQUEST_PTR: u32 = 0x39AC_3044;
pub const REQUEST_PI: f32 = core::f32::consts::PI;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MovementRequest {
    /// `+0x00`.
    pub flags: u32,
    /// `+0x04` external move-to direction (`FUN_39402fe0`).
    pub external_dir: [f32; 3],
    /// `+0x10`.
    pub pad_10: [u8; 0x10],
    /// `+0x20` set to 1.0 by the external-target path.
    pub scale_20: f32,
    /// `+0x24` constructor `-1.0`.
    pub minus_one_24: f32,
    /// `+0x28` constructor `-1.0`.
    pub minus_one_28: f32,
    /// `+0x2c` movement type (`FUN_394037d0`).
    pub movement_type: i32,
    /// `+0x30` desired input velocity.
    pub velocity: [f32; 3],
    /// `+0x3c` external delta block.
    pub delta2: [f32; 3],
    /// `+0x48`.
    pub pad_48: [u8; 0x14],
    /// `+0x5c` constructor `1.0`.
    pub one_5c: f32,
    /// `+0x60`.
    pub pad_60: u32,
    /// `+0x64` module pointer `0x39ac3044`.
    pub ptr_64: u32,
    /// `+0x68`.
    pub pad_68: u32,
    /// `+0x6c` constructor `-1.0`.
    pub minus_one_6c: f32,
    /// `+0x70` constructor `pi`.
    pub pi_70: f32,
    /// `+0x74` constructor `0.5`.
    pub half_74: f32,
    /// `+0x78` constructor `-1.0`.
    pub minus_one_78: f32,
    /// `+0x7c` constructor u16 `1`.
    pub one_7c: u16,
    /// `+0x7e` constructor byte `0`; `+0x7f` is uninitialised stack in the original.
    pub pad_7e: [u8; 2],
    /// `+0x80` module pointer `0x39ac3044`.
    pub ptr_80: u32,
    /// `+0x84` constructor `-1`.
    pub minus_one_84: u32,
    /// `+0x88`.
    pub pad_88: [u8; 0x0c],
    /// `+0x94` controlled-unit kind (0 normal, 2 mount/mate).
    pub unit_kind: u32,
    /// `+0x98` pseudo-speed (0 idle, 1.0 moving, 0.4 walking).
    pub pseudo_speed: f32,
}

const _: () = assert!(core::mem::size_of::<MovementRequest>() == REQUEST_SIZE);

impl Default for MovementRequest {
    fn default() -> Self {
        Self {
            flags: 0,
            external_dir: [0.0; 3],
            pad_10: [0; 0x10],
            scale_20: 0.0,
            minus_one_24: 0.0,
            minus_one_28: -1.0,
            movement_type: 0,
            velocity: [0.0; 3],
            delta2: [0.0; 3],
            pad_48: [0; 0x14],
            one_5c: 1.0,
            pad_60: 0,
            ptr_64: DEFAULT_REQUEST_PTR,
            pad_68: 0,
            minus_one_6c: -1.0,
            pi_70: REQUEST_PI,
            half_74: 0.5,
            minus_one_78: -1.0,
            one_7c: 1,
            pad_7e: [0; 2],
            ptr_80: DEFAULT_REQUEST_PTR,
            minus_one_84: 0xffff_ffff,
            pad_88: [0; 0x0c],
            unit_kind: 0,
            pseudo_speed: 0.0,
        }
    }
}

impl MovementRequest {
    pub fn from_bytes(bytes: &[u8; REQUEST_SIZE]) -> Self {
        unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const Self) }
    }

    pub fn to_bytes(self) -> [u8; REQUEST_SIZE] {
        unsafe { core::ptr::read_unaligned(&self as *const Self as *const [u8; REQUEST_SIZE]) }
    }
}
