//! `CAnimationGraph::SStateInfo::EvaluateGuards` (`cryaction.dll` `FUN_305f7a80`, 356 B).
//!
//! The state stores a byte guard count at `+0xb` and `u16` guards at `+0x10`. Each guard packs
//! `input = g & 0x1f`, `value = (g >> 5) & 0xff`, `op = (g >> 13) & 3`. The current input is the
//! encoded `u16` from the graph state's input bank (`+0x2e + bank*0x40`). The original returns 0
//! as soon as a guard fails and 1 otherwise; op 3 is not handled by the switch and therefore
//! passes, exactly like the decompiled `if/else` chain.

/// Port of `FUN_305f7a80`.
///
/// `guards` is the state's guard array (`state+0x10`, `state+0xb` entries); `encoded` is the
/// 32-entry encoded input bank. The original reads `encoded[input]` without a bounds check; the
/// input field is 5 bits, so the array is always 32 entries.
pub fn evaluate_guards(guards: &[u16], encoded: &[u16; 32]) -> bool {
    for &g in guards {
        let input = (g & 0x1f) as usize;
        let value = (g >> 5) & 0xff;
        let op = (g >> 13) & 3;
        let current = encoded[input];
        match op {
            0 if value < current => return false,
            1 if current < value => return false,
            2 if current == value => return false,
            _ => {}
        }
    }
    true
}
