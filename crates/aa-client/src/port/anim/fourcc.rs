//! `FUN_31568fc0` (`cryanimation.dll`, 97 B): locomotion blend-type FOURCC -> switch case.
//!
//! The original binary-searches a 58-entry sorted table at `cryanimation+0x1b5890` (dumped from
//! the live 1.2 client). An unknown FOURCC logs `locomotion-group '%s' not identified. This a
//! fatal-error.` and returns `0x3a` (58), the `default` case of `GetLMGCapabilities`.
//!
//! The table is the *runtime* table, not the one in the on-disk DLL: `.data` is initialised at
//! startup and the shipped file has only the zero-filled virtual tail for this range.

/// `{FOURCC, case}` in ascending FOURCC order, exactly as at `cryanimation+0x1b5890`.
pub const FOURCC_TABLE: &[(u32, u16)] = &[
    (0x30314c58, 51), // XL10
    (0x3130494d, 38), // MI01
    (0x31314c58, 52), // XL11
    (0x31465453, 0),  // STF1
    (0x31484455, 1),  // UDH1
    (0x3149324d, 2),  // M2I1
    (0x314d3249, 3),  // I2M1
    (0x314e5254, 4),  // TRN1
    (0x31524c46, 5),  // FLR1
    (0x31564c58, 40), // XLV1
    (0x315f3853, 35), // S8_1
    (0x315f5f4d, 7),  // M__1
    (0x315f5f53, 6),  // S__1
    (0x3230494d, 39), // MI02
    (0x32314c58, 53), // XL12
    (0x32445553, 8),  // SUD2
    (0x32465453, 9),  // STF2
    (0x32484455, 10), // UDH2
    (0x32485453, 11), // STH2
    (0x3248545f, 34), // _TH2
    (0x32485f53, 12), // S_H2
    (0x324d3249, 13), // I2M2
    (0x324d4958, 14), // XIM2
    (0x32524c46, 15), // FLR2
    (0x32564c58, 41), // XLV2
    (0x32574657, 16), // WFW2
    (0x325f5453, 17), // ST_2
    (0x325f5f53, 18), // S__2
    (0x33314c58, 54), // XL13
    (0x33484455, 19), // UDH3
    (0x33523853, 36), // S8R3
    (0x33524c46, 20), // FLR3
    (0x33564c58, 42), // XLV3
    (0x33574652, 21), // RFW3
    (0x34314c58, 55), // XL14
    (0x344d3249, 47), // I2M4
    (0x34564c58, 43), // XLV4
    (0x35314c58, 56), // XL15
    (0x35564c58, 44), // XLV5
    (0x36564c58, 45), // XLV6
    (0x37564c58, 48), // XLV7
    (0x3849324d, 37), // M2I8
    (0x38564c58, 49), // XLV8
    (0x39564c58, 50), // XLV9
    (0x41495f54, 22), // T_IA
    (0x424d4c43, 23), // CLMB
    (0x48535550, 24), // PUSH
    (0x4c414353, 25), // SCAL
    (0x504f4f43, 26), // COOP
    (0x50545349, 27), // ISTP
    (0x50545354, 28), // TSTP
    (0x53495f54, 29), // T_IS
    (0x53575f54, 30), // T_WS
    (0x544f5249, 31), // IROT
    (0x544f5250, 32), // PROT
    (0x57533853, 57), // S8SW
    (0x58485453, 33), // STHX
    (0x584d3249, 46), // I2MX
];

/// `FUN_31568fc0`. The binary search is reproduced instruction-for-instruction:
/// `mid = (lo + hi) >> 1`, `lo`/`hi` are wrapping `u32`s, and the sentinel case is `0x3a`.
pub fn fourcc_case(fourcc: u32) -> u16 {
    // The original's loop bounds are signed ints even though the key compare is unsigned:
    // `hi = mid - 1` becomes -1 and ends the loop (the decompiler showed `uint`, but the
    // unsigned version would index past the table on an unknown low key).
    let mut lo: i32 = 0;
    let mut hi: i32 = 0x3a;
    while lo <= hi {
        let mid = (hi + lo) >> 1;
        let key = FOURCC_TABLE[mid as usize].0;
        if fourcc < key {
            hi = mid - 1;
        } else if fourcc > key {
            lo = mid + 1;
        } else {
            return FOURCC_TABLE[mid as usize].1;
        }
    }
    0x3a
}
