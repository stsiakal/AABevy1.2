//! CryAnimation locomotion-group blend functions, step 4 of the animation-graph port.
//!
//! Each function is a line-by-line port of the 1.2 `cryanimation.dll` body named in its doc
//! comment. They fill the 0x58-byte LMG capabilities block (`Caps`) used by
//! `CAnimationSet::GetAnimationDesc FUN_31558f80`, from
//!
//! * the animation-set table (`AnimSet`: id -> `0xdc`-byte animation data row),
//! * the animation descriptor (`Desc`, the `SAnimation` object: ids at `+4`, direction at
//!   `+0x148`/`+0x14c`, extra blend parameters at `+0x144`, `+0x150`, `+0x154`),
//! * the LMG example list (`Lmg`, 8 rows of `0x2c` bytes; position at `+0x1c`/`+0x20`).
//!
//! The buffer types deliberately keep the original's byte offsets. The TSTP body and the shared
//! S8_1/M2I8/MI01/MI02/S8SW body (including the hardcoded fallback directions) are bit-exact
//! against the live client (`goldens/lmg_weights.json`, `goldens/lmg_s8_fallback.json`).

/// `DAT_316745a0`: 1.0.
pub const ONE: f32 = 1.0;
/// `DAT_316745e0`: `0x7fffffff`, the absolute-value mask used by the original.
pub const ABS_MASK: u32 = 0x7fff_ffff;
/// `DAT_316745f0`: 0.1, the "LMG has no example positions" threshold.
pub const POSITION_EPS: f32 = 0.1;
/// `_DAT_31676e18`: the barycentric minimum sum.
pub const BARY_EPS: f32 = 1.192_092_9e-7;
/// `DAT_3167818c`: the 8-way fallback weight (1/8).
pub const EIGHTH: f32 = 0.125;
/// `DAT_31676a00`: 0.2, written to `Caps+0x54` and `Caps+0x28` by the per-type bodies.
pub const CAPS_54: f32 = 0.2;

/// The 0x58-byte capabilities block filled by `GetLMGCapabilities FUN_315714b0`.
#[derive(Clone, Debug, PartialEq)]
pub struct Caps(pub [u8; 0x58]);

impl Default for Caps {
    fn default() -> Self {
        Self([0; 0x58])
    }
}

impl Caps {
    /// The state `GetLMGCapabilities FUN_315714b0` leaves before dispatching to a type body:
    /// `+0x10 = -1`, `+0x14 = 8`, `+0x18 = 2*pi*4`, `+0x20/+0x24 = -1`, everything else zero.
    pub fn init_lmg() -> Self {
        let mut caps = Self::default();
        caps.set_f32(0x10, -1.0);
        caps.set_f32(0x14, 8.0);
        caps.set_f32(0x18, 25.132_742);
        caps.set_f32(0x20, -1.0);
        caps.set_f32(0x24, -1.0);
        caps
    }
    pub fn u8_at(&self, off: usize) -> u8 {
        self.0[off]
    }
    pub fn f32_at(&self, off: usize) -> f32 {
        f32_le(&self.0, off)
    }
    fn set_u8(&mut self, off: usize, value: u8) {
        self.0[off] = value;
    }
    fn set_u16(&mut self, off: usize, value: u16) {
        self.0[off..off + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn set_f32(&mut self, off: usize, value: f32) {
        self.0[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }
    /// The original writes a whole 64-bit zero-extended slot (`f32` at `off`, zero at `off+4`).
    fn set_f32_zero_pair(&mut self, off: usize, value: f32) {
        self.set_f32(off, value);
        self.0[off + 4..off + 8].fill(0);
    }
}

/// One animation-set entry: `{?, ?, row index, ?}`; the original reads the row index at `+8`.
pub type AnimEntry = [u8; 0x10];
/// One `0xdc`-byte animation data row (speeds, direction pairs, ...).
pub type AnimRow = [u8; 0xdc];

/// A synthetic or captured animation set: `entries[id]` -> `rows[entry[8..12]]`.
#[derive(Clone, Debug, Default)]
pub struct AnimSet {
    pub entries: Vec<AnimEntry>,
    pub rows: Vec<AnimRow>,
}

impl AnimSet {
    pub fn row_index(&self, id: i16) -> usize {
        let entry = &self.entries[id as usize];
        u32_le(entry, 8) as usize
    }
    pub fn row(&self, id: i16) -> &AnimRow {
        &self.rows[self.row_index(id)]
    }
}

/// The `SAnimation` object read by the blend functions.
pub type Desc = [u8];

/// The LMG example data; `examples[i * 0x2c ..]` is one example.
#[derive(Clone, Debug, Default)]
pub struct Lmg {
    pub examples: Vec<u8>,
}

impl Lmg {
    fn at(&self, row: usize, off: usize) -> f32 {
        f32_le(&self.examples, row * 0x2c + off)
    }
}

pub fn f32_le(buf: &[u8], off: usize) -> f32 {
    f32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

pub fn u16_le(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

pub fn i16_le(buf: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([buf[off], buf[off + 1]])
}

pub fn u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn desc_f32(desc: &Desc, off: usize) -> f32 {
    f32_le(desc, off)
}

fn desc_id(desc: &Desc, index: usize) -> i16 {
    i16_le(desc, 4 + index * 2)
}

fn desc_dir(desc: &Desc) -> (f32, f32) {
    (desc_f32(desc, 0x148), desc_f32(desc, 0x14c))
}

/// `FUN_31569030`: barycentric pair weights along the segment `a -> b`.
fn barycentric(a: (f32, f32), b: (f32, f32), dir: (f32, f32)) -> (f32, f32) {
    let denom = a.0 * b.1 - b.0 * a.1;
    let inv = ONE / denom;
    let w1 = (dir.0 * b.1 - dir.1 * b.0) * inv;
    let w2 = (dir.1 * a.0 - dir.0 * a.1) * inv;
    if BARY_EPS < w2 + w1 {
        let scale = ONE / (w2 + w1);
        (scale * w1, scale * w2)
    } else if w2 <= w1 {
        (ONE, ONE - w1)
    } else {
        (0.0, ONE - w1)
    }
}

fn cross(dir: (f32, f32), point: (f32, f32)) -> f32 {
    dir.1 * point.0 - dir.0 * point.1
}

/// `FUN_31569a20`: 8-way directional blend over the example points `p0..p7`.
pub fn fun_31569a20(points: &[(f32, f32); 8], dir: (f32, f32)) -> [f32; 8] {
    let mut out = [0.0f32; 8];
    let (dx, dy) = dir;
    if dx * dx + dy * dy == 0.0 {
        return [EIGHTH; 8];
    }
    let c: [f32; 8] = std::array::from_fn(|i| cross(dir, points[i]));
    // Sector topology of the original: points (p0,p1), (p1,p3), (p3,p5), (p5,p4),
    // (p4,p6), (p6,p7), (p7,p2), (p2,p0); the two results land in slots (i,i+1 mod 8).
    let sectors: [(usize, usize, usize, usize, bool); 8] = [
        (0, 1, 0, 1, false),
        (1, 3, 1, 2, false),
        (3, 5, 2, 3, false),
        (5, 4, 3, 4, false),
        (4, 6, 4, 5, true),
        (6, 7, 5, 6, false),
        (7, 2, 6, 7, false),
        (2, 0, 7, 0, false),
    ];
    for (a, b, oa, ob, strict) in sectors {
        if c[a] >= 0.0 && c[b] <= 0.0 && (!strict || c[a] != c[b]) {
            let (wa, wb) = barycentric(points[a], points[b], dir);
            out[oa] = wa;
            out[ob] = wb;
            return out;
        }
    }
    [EIGHTH; 8]
}

/// The eight fallback directions used when the LMG has no example positions.
const FALLBACK: [(f32, f32); 8] = [
    (0.0, 1.0),
    (-1.0, 1.0),
    (1.0, 1.0),
    (-1.0, 0.0),
    (0.0, -1.0),
    (-1.0, -1.0),
    (1.0, -1.0),
    (1.0, 0.0),
];

/// `FUN_31574e70`: the shared `S8_1`/`M2I8`/`MI01`/`MI02`/`S8SW` body.
///
/// `base` is the original's `param_3` (example base, 0.0 for every shipped caller).
pub fn fun_31574e70(set: &AnimSet, desc: &Desc, base: f32, lmg: &Lmg, caps: &mut Caps) {
    caps.set_f32(0x54, CAPS_54);
    caps.set_f32(0x50, ONE);
    let index = base as usize;
    let ids: [i16; 8] = std::array::from_fn(|i| desc_id(desc, index + i));
    let rows: [&AnimRow; 8] = std::array::from_fn(|i| set.row(ids[i]));

    let first = (lmg.at(index, 0x1c), lmg.at(index, 0x20));
    let mut points = FALLBACK;
    let mut speeds = [0.0f32; 8];
    if first.0 * first.0 + first.1 * first.1 <= POSITION_EPS {
        for i in 0..8 {
            speeds[i] = f32_le(rows[i], 0xa8);
        }
    } else {
        for i in 0..8 {
            let p = (lmg.at(index + i, 0x1c), lmg.at(index + i, 0x20));
            points[i] = p;
            speeds[i] = (p.0 * p.0 + p.1 * p.1).sqrt();
        }
    }

    caps.set_u8(0, 1);
    caps.set_u8(1, 1);
    caps.set_u8(2, 1);
    let w = fun_31569a20(&points, desc_dir(desc));
    let speed_sum = w[1] * speeds[1]
        + w[0] * speeds[0]
        + w[7] * speeds[2]
        + w[2] * speeds[3]
        + w[4] * speeds[4]
        + w[3] * speeds[5]
        + w[5] * speeds[6]
        + w[6] * speeds[7];
    let rate = w[1] * f32_le(rows[1], 0x30)
        + w[0] * f32_le(rows[0], 0x30)
        + w[7] * f32_le(rows[2], 0x30)
        + w[2] * f32_le(rows[3], 0x30)
        + w[4] * f32_le(rows[4], 0x30)
        + w[3] * f32_le(rows[5], 0x30)
        + w[5] * f32_le(rows[6], 0x30)
        + w[6] * f32_le(rows[7], 0x30);
    caps.set_f32(0x20, rate);
    caps.set_f32(0x24, rate);
    caps.set_f32_zero_pair(0x28, speed_sum);
    caps.set_f32_zero_pair(0x34, speed_sum);
    caps.set_u8(6, 1);
    caps.set_u32_zero(0x30);
    caps.set_u32_zero(0x3c);
}

impl Caps {
    fn set_u32_zero(&mut self, off: usize) {
        self.0[off..off + 4].fill(0);
    }
}

/// `FUN_3156bf40`: `TSTP` (turn in place). The caps carry the animation speed spans only.
pub fn fun_3156bf40(set: &AnimSet, desc: &Desc, lmg: &Lmg, caps: &mut Caps) {
    let _ = set;
    let mut hi = -99999.0f32;
    let mut lo = 99999.0f32;
    let mut hi_speed = -99999.0f32;
    let mut lo_speed = 99999.0f32;
    let count = lmg.example_count();
    for i in 0..count {
        let id = desc_id(desc, i);
        let row = set.row(id);
        let speed = f32_le(row, 0xb4);
        let rate = f32_le(row, 0x30);
        hi = hi.max(speed);
        lo = lo.min(speed);
        hi_speed = hi_speed.max(rate);
        lo_speed = lo_speed.min(rate);
    }
    let _ = lo_speed;
    caps.set_u16(0, 0x0101);
    caps.set_u8(2, 0);
    caps.set_u8(4, 1);
    caps.set_u8(6, 0);
    caps.0[8..12].fill(0);
    caps.set_u32_zero(0x10);
    caps.set_u32_zero(0x14);
    caps.set_f32(0x20, hi_speed);
    caps.set_f32(0x24, hi_speed);
    caps.set_f32_zero_pair(0x28, 0.0);
    caps.set_u32_zero(0x30);
    caps.set_f32_zero_pair(0x34, 0.0);
    caps.set_u32_zero(0x3c);
    caps.set_f32(0x48, hi);
    caps.set_f32(0x4c, lo);
    caps.set_f32(0x50, 0.0);
    caps.set_f32(0x54, 0.0);
}

impl Lmg {
    /// `*(uint *)(lmg->examples - 4) & 0x3fffffff`.
    pub fn example_count(&self) -> usize {
        self.examples.len() / 0x2c
    }
}

// The XL14/XL15/ISTP bodies are intentionally not present yet: their angle helpers call
// libm `atan2` with two float vectors and the surviving goldens for them still need the
// original capability calls. See `D-graph-runtime.md` §7 for the open list.
