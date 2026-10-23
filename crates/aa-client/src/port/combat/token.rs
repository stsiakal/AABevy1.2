//! Unit-token parser `FUN_3945fcd0` (VA `0x3945fcd0`, 1003 bytes).
//!
//! The original returns a 64-bit value: `EAX` is the kind (`1` unit, `2` team member object,
//! `3` resolved object, `0` failure), `EDX` is the pointer.  It recognises these prefixes
//! (case-sensitive `XlComparePrefix`, extra text after the matched prefix is ignored unless it is
//! a `target` suffix):
//!
//! | prefix | resolution |
//! |---|---|
//! | `player` | local actor (`DAT_39b02294+0x10c`) |
//! | `playerpet` | pet actor's `+0x20` id -> `FUN_39360020` |
//! | `target` | local actor's current target (`+0x1aa0`); on failure the focus item (`+0x5f90` -> `FUN_39035110`) |
//! | `watchtarget` | local actor's watch target (`+0x1aa4`) |
//! | `team<n>` | `FUN_39307320(n-1)`; a member without a unit is returned as kind 2 |
//! | `loginch<n>` | `FUN_39172980(n)` |
//! | `previewch<a><b>` | `FUN_39170df0(a, atoi(text+10))` (the second number starts one byte after the first) |
//! | `house` | `FUN_39325110()` id -> `FUN_39360020` |
//! | `doodad` | `FUN_393b28b0()` id -> `FUN_39035110` (kind 3) |
//!
//! After a unit is resolved, every trailing `target` suffix moves to that unit's `+0x1aa0`
//! target id (`FUN_39360020`); an empty/unresolved id ends the whole parse with kind 0.  If a
//! fallback object was also found (the `target`/`doodad` paths), a remaining `target` suffix is an
//! error; otherwise the unit wins.  Without a unit, kind 2 (team object) wins over kind 3
//! (object).

pub const TOKEN_KIND_NONE: u32 = 0;
pub const TOKEN_KIND_UNIT: u32 = 1;
pub const TOKEN_KIND_TEAM_MEMBER: u32 = 2;
pub const TOKEN_KIND_OBJECT: u32 = 3;

/// `DAT_3a5aa29c` / `DAT_3a1776e0` / `DAT_3a5aa2b4` are all zero in the 1.2 image.
pub const EMPTY_TARGET_ID: u32 = 0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenRef {
    pub kind: u32,
    pub ptr: u32,
}

impl TokenRef {
    pub const fn none() -> Self {
        Self { kind: TOKEN_KIND_NONE, ptr: 0 }
    }
}

/// One unit as seen through the small world table used by the parser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenUnit {
    /// Object id (`unit+0x8`), the key used by `FUN_39360020`.
    pub id: u32,
    /// Opaque object pointer returned by the parser.
    pub ptr: u32,
    /// `unit+0x1aa0`: current target id (0 = none).
    pub target: u32,
    /// `unit+0x1aa4`: watch target id (only read on the local unit).
    pub watch: u32,
}

/// A party row from `FUN_39307320`; `unit` is the result of `FUN_3935e370(*row)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeamMember {
    pub ptr: u32,
    pub unit: Option<TokenUnit>,
}

/// The live state the parser reads, reduced to plain tables.
#[derive(Clone, Debug, Default)]
pub struct TokenWorld {
    pub local: Option<TokenUnit>,
    /// Local actor `+0x5f90` resolved through `FUN_39035110` (object pointer).
    pub local_focus_item: Option<u32>,
    pub pet: Option<TokenUnit>,
    /// `FUN_39307320(index)` rows, index 0 is party member 1.
    pub team: Vec<Option<TeamMember>>,
    /// `FUN_39172980(index)` characters.
    pub login: Vec<Option<TokenUnit>>,
    /// `FUN_39170df0(first, second)` preview characters.
    pub preview: Vec<(i32, i32, TokenUnit)>,
    /// `FUN_39325110()` house id, looked up through `FUN_39360020`.
    pub house: Option<u32>,
    /// `FUN_39035110(FUN_393b28b0())` doodad object pointer.
    pub doodad: Option<u32>,
    /// `FUN_39360020` unit table.
    pub units: Vec<TokenUnit>,
}

impl TokenWorld {
    fn lookup(&self, id: u32) -> Option<TokenUnit> {
        self.units.iter().find(|unit| unit.id == id).copied()
    }
}

/// Parse one token.  `None` is the original's NULL argument; an empty string is its
/// "unitText is empty" path.  Both return kind 0.
pub fn parse_token(world: &TokenWorld, text: Option<&str>) -> TokenRef {
    let Some(local) = world.local else {
        // "no client player exists"
        return TokenRef::none();
    };
    let Some(text) = text else {
        // "unitText is NULL"
        return TokenRef::none();
    };
    if text.is_empty() {
        // "unitText is empty"
        return TokenRef::none();
    }

    let mut unit: Option<TokenUnit> = None;
    let mut object: Option<u32> = None;
    let mut team_member: Option<u32> = None;
    let mut rest: &str;

    if let Some(tail) = text.strip_prefix("playerpet") {
        unit = world.pet;
        rest = tail;
    } else if let Some(tail) = text.strip_prefix("player") {
        unit = Some(local);
        rest = tail;
    } else if let Some(tail) = text.strip_prefix("target") {
        if local.target != EMPTY_TARGET_ID {
            unit = world.lookup(local.target);
        }
        if unit.is_none() {
            object = world.local_focus_item;
        }
        rest = tail;
    } else if let Some(tail) = text.strip_prefix("watchtarget") {
        if local.watch != EMPTY_TARGET_ID {
            unit = world.lookup(local.watch);
        }
        rest = tail;
    } else if let Some(tail) = text.strip_prefix("team") {
        let index = atoi(tail);
        if !(1..50).contains(&index) {
            return TokenRef::none();
        }
        let Some(member) = world
            .team
            .get((index - 1) as usize)
            .and_then(|member| *member)
        else {
            return TokenRef::none();
        };
        team_member = Some(member.ptr);
        match member.unit {
            Some(member_unit) => {
                unit = Some(member_unit);
                rest = tail;
            }
            // `FUN_3935e370` failed: the team object is returned directly, no suffix chaining.
            None => {
                return TokenRef {
                    kind: TOKEN_KIND_TEAM_MEMBER,
                    ptr: member.ptr,
                };
            }
        }
    } else if let Some(tail) = text.strip_prefix("loginch") {
        let index = atoi(tail);
        unit = if index >= 0 {
            world.login.get(index as usize).and_then(|entry| *entry)
        } else {
            None
        };
        rest = tail;
    } else if let Some(tail) = text.strip_prefix("previewch") {
        let first = atoi(tail);
        let second = atoi(tail.get(1..).unwrap_or(""));
        unit = world
            .preview
            .iter()
            .find(|(a, b, _)| *a == first && *b == second)
            .map(|(_, _, unit)| *unit);
        rest = tail;
    } else if let Some(tail) = text.strip_prefix("house") {
        unit = world.house.and_then(|id| world.lookup(id));
        rest = tail;
    } else if let Some(tail) = text.strip_prefix("doodad") {
        object = world.doodad;
        rest = tail;
    } else {
        return TokenRef::none();
    }

    if let Some(mut current) = unit {
        while !rest.is_empty() && rest.starts_with("target") {
            if current.target == EMPTY_TARGET_ID {
                return TokenRef::none();
            }
            let Some(next) = world.lookup(current.target) else {
                return TokenRef::none();
            };
            current = next;
            rest = &rest[6..];
        }
        if object.is_some() && rest.starts_with("target") {
            return TokenRef::none();
        }
        return TokenRef {
            kind: TOKEN_KIND_UNIT,
            ptr: current.ptr,
        };
    }

    if object.is_some() && rest.starts_with("target") {
        return TokenRef::none();
    }
    if let Some(ptr) = team_member {
        return TokenRef {
            kind: TOKEN_KIND_TEAM_MEMBER,
            ptr,
        };
    }
    if let Some(ptr) = object {
        return TokenRef {
            kind: TOKEN_KIND_OBJECT,
            ptr,
        };
    }
    TokenRef::none()
}

/// C `atoi` for the ASCII numeric suffixes this parser accepts.  The original reads past a
/// shorter suffix only until the NUL; indices this small never overflow.
fn atoi(text: &str) -> i32 {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let mut sign = 1i64;
    if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
        if bytes[index] == b'-' {
            sign = -1;
        }
        index += 1;
    }
    let mut value = 0i64;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        value = (value * 10 + i64::from(bytes[index] - b'0')).min(i64::from(i32::MAX) + 1);
        index += 1;
    }
    (value * sign).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
