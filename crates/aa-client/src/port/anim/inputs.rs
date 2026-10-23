//! Input encoders: `CIntegerInputValue::EncodeInput` (`cryaction+0x604b10`), the int virtual
//! `+0x604c50`, `CFloatInputValue::EncodeInput` (`+0x605180`), the float virtual `+0x6052d0`,
//! and the keyed encoders `+0x611470` / `+0x6115d0`.
//!
//! The string forms use `sscanf("%d")` / `sscanf("%f")` and return `0xffff` when parsing fails.
//! The numeric forms clamp to the input object's integer range, then return the index of the
//! range that contains the value (`0xffff` when none does). Floats first map
//! `(offset + value) / scale` and truncate to an int (`cvttss2si` semantics: NaN and
//! out-of-range become `INT_MIN`).

/// Range list (`+0x18..+0x1c`, pairs of inclusive int bounds) stored in the input object.
pub type Ranges = [(i32, i32)];

fn find_range(ranges: &Ranges, value: i32) -> u16 {
    for (i, &(lo, hi)) in ranges.iter().enumerate() {
        if lo <= value && value <= hi {
            return i as u16;
        }
    }
    0xffff
}

/// `FUN_30604c50`: integer encode. `min`/`max` are the object's `+0x28`/`+0x2c`; the original
/// clamps `value` to `[min, max]` (with `max <= value` taking `max`) and scans the ranges.
pub fn encode_int(ranges: &Ranges, min: i32, max: i32, value: i32) -> u16 {
    let mut value = value;
    if value < min {
        value = min;
    } else if max <= value {
        value = max;
    }
    find_range(ranges, value)
}

/// `FUN_30604b10`: string wrapper for the integer encode. `sscanf` failure -> `0xffff`.
pub fn encode_int_str(ranges: &Ranges, min: i32, max: i32, text: &str) -> u16 {
    match parse_int(text) {
        Some(value) => encode_int(ranges, min, max, value),
        None => 0xffff,
    }
}

/// `FUN_306052d0`: float encode.
///
/// `f = (offset + value) / scale` (all `f32`, the original's exact op order); clamp to the
/// integer `[min, max]` as floats; `(int)f` truncates toward zero; scan the ranges.
pub fn encode_float(
    ranges: &Ranges,
    min: i32,
    max: i32,
    scale: f32,
    offset: f32,
    value: f32,
) -> u16 {
    let mut f = (offset + value) / scale;
    let min_f = min as f32;
    if f < min_f {
        f = min_f;
    } else {
        let max_f = max as f32;
        if max_f <= f {
            f = max_f;
        }
    }
    find_range(ranges, f32_to_i32_trunc(f))
}

/// `FUN_30605180`: string wrapper for the float encode. `sscanf` failure -> `0xffff`.
pub fn encode_float_str(
    ranges: &Ranges,
    min: i32,
    max: i32,
    scale: f32,
    offset: f32,
    text: &str,
) -> u16 {
    match parse_float(text) {
        Some(value) => encode_float(ranges, min, max, scale, offset, value),
        None => 0xffff,
    }
}

/// `FUN_306115d0`: keyed encode by name. The object's sorted key list is searched by the same
/// strcmp-style loop; a missing key returns `0xffff`.
pub fn encode_key(keys: &[(impl AsRef<str>, u16)], name: &str) -> u16 {
    for (key, index) in keys {
        if key.as_ref() == name {
            return *index;
        }
    }
    0xffff
}

/// `FUN_30611470`: keyed encode by local key index. `keys[local]` names the key; the result is
/// the key's graph-wide index, or `0xffff` when the local index is out of range or the name is
/// not present in the graph key map. The original writes the result into `out[input_id]`; the
/// caller supplies the id, so the returned value is enough here.
pub fn encode_key_index(keys: &[(impl AsRef<str>, u16)], local: usize) -> u16 {
    keys.get(local).map(|(_, index)| *index).unwrap_or(0xffff)
}

/// `cvttss2si`: truncate toward zero, `INT_MIN` for NaN/out-of-range (x86 semantics). Rust's
/// `as i32` saturates instead, so use the SSE instruction where available.
pub fn f32_to_i32_trunc(value: f32) -> i32 {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::{_mm_cvtt_ss2si, _mm_set_ss};
        unsafe { _mm_cvtt_ss2si(_mm_set_ss(value)) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        if value.is_nan() || value > i32::MAX as f32 || value < i32::MIN as f32 {
            i32::MIN
        } else {
            value as i32
        }
    }
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r')
}

/// `sscanf("%d")` for the cases the graph data contains: optional leading whitespace, optional
/// sign, decimal digits, stop at the first non-digit. Returns `None` when no digit is consumed.
pub fn parse_int(text: &str) -> Option<i32> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && is_space(bytes[i]) {
        i += 1;
    }
    let mut negative = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        negative = bytes[i] == b'-';
        i += 1;
    }
    let start = i;
    let mut value: i64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = value
            .saturating_mul(10)
            .saturating_add((bytes[i] - b'0') as i64);
        i += 1;
    }
    if i == start {
        return None;
    }
    if negative {
        value = -value;
    }
    Some(value.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

/// `sscanf("%f")`: whitespace, sign, decimal mantissa with optional fraction and exponent.
/// The live client's MSVC `sscanf` rejects `inf`/`nan` (the golden returns `0xffff`), so this
/// parser does too. Returns `None` when no conversion is performed.
pub fn parse_float(text: &str) -> Option<f32> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && is_space(bytes[i]) {
        i += 1;
    }
    let start = i;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let mut saw_digit = false;
    let mut saw_dot = false;
    let mut j = i;
    while j < bytes.len() {
        let b = bytes[j];
        if b.is_ascii_digit() {
            saw_digit = true;
            j += 1;
        } else if b == b'.' && !saw_dot {
            saw_dot = true;
            j += 1;
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }
    if j < bytes.len() && (bytes[j] == b'e' || bytes[j] == b'E') {
        let mut k = j + 1;
        if k < bytes.len() && (bytes[k] == b'+' || bytes[k] == b'-') {
            k += 1;
        }
        let exp_start = k;
        while k < bytes.len() && bytes[k].is_ascii_digit() {
            k += 1;
        }
        if k > exp_start {
            j = k;
        }
    }
    let token = &text[start..j];
    token.parse::<f32>().ok()
}
