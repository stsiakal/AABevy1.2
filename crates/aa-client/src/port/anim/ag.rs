//! Compiled animation-graph loader (`Animations/graphs/*.ag`).
//!
//! The 1.2 loader opens the compiled side copy before the XML whenever one exists:
//! `CAnimationGraphManager::LoadGraph` (`cryaction+0x6261f0`) strips the XML extension,
//! appends `.ag` (`DAT_307435ec`) and calls the graph's serialization virtual
//! (`CAnimationGraph::Serialize`, vtable `0x30741dcc+0x20` = `FUN_306152b0`, reader branch
//! with `"rb"`). The file begins with version `0x31`.
//!
//! Sections after the header (all little-endian):
//!
//! 1. factory-node graph (`FUN_30623770` / per-class `+0x1c`); not needed for state
//!    selection and therefore skipped by seeking the serialized input table;
//! 2. input table (`CFloatInputValue`/`CIntegerInputValue`/`CKeyedInputValue`
//!    `+0x30` serializers) — exactly `FUN_30601060` + the object virtuals;
//! 3. nine timing words and a u16 (`FUN_305eb280(+0x30,9)`);
//! 4. guard interval map (`FUN_3060ed70`);
//! 5. states (`FUN_30606dc0`, 0xa0 bytes in memory, 17+16+8 serialized bytes);
//! 6. transitions (`FUN_30607cd0`, `{u16,u16,u16,u16,u16,f32}`);
//! 7. state-name map (`FUN_3060cff0`);
//! 8. `CStateIndex` (`FUN_3065bc10`, entries via `FUN_30658930`);
//! 9. hub (`+0x180`), `+0x182`, the two string maps, the triple table, `+0x1e0`
//!    and the final map (`FUN_3060ece0`/`FUN_30611020`/`FUN_3060ed70`).
//!
//! The input table is located by trying the structural parser at each byte offset and
//! accepting the single offset that consumes the whole file with consistent state,
//! transition, name-map and `CStateIndex` counts. This keeps the port independent of the
//! factory-node classes while still validating every table the graph state machine uses.

use super::graph::{
    AnimationGraph, GraphInput, GraphSource, InputKind, StateIndex, StateIndexEntry, StateInfo,
    TransitionInfo,
};

const VERSION: u32 = 0x31;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    fn u8(&mut self) -> Option<u8> {
        let v = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }

    fn u16(&mut self) -> Option<u16> {
        let b = self.data.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Option<u32> {
        let b = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.u32()?))
    }

    /// `FUN_305047b0`: u32 length including the trailing NUL, then the bytes.
    fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        if len == 0 || len > 4096 {
            return None;
        }
        let b = self.data.get(self.pos..self.pos + len)?;
        self.pos += len;
        if b[len - 1] != 0 || b[..len - 1].contains(&0) {
            return None;
        }
        Some(String::from_utf8_lossy(&b[..len - 1]).into_owned())
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        self.data.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(())
    }
}

/// Load `game/animations/graphs/<name>.ag` from the 1.2 `game_pak`.
pub fn load_graph_ag_from_pak(name: &str) -> Result<AnimationGraph, String> {
    let path = ag_path(name);
    let data = aa_pak::Pak::global()
        .read(&path)
        .map_err(|e| format!("{path}: {e}"))?;
    load_graph_ag(name, &data)
}

/// The compiled path for a graph name: strip the XML extension, append `.ag`
/// (`FUN_306261f0`).
pub fn ag_path(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    format!("game/animations/graphs/{stem}.ag")
}

/// Parse a compiled `.ag` file. `name` is the logical graph name (without `.ag`).
pub fn load_graph_ag(name: &str, data: &[u8]) -> Result<AnimationGraph, String> {
    let version = u32::from_le_bytes(
        data.get(0..4)
            .ok_or_else(|| format!("{name}: truncated .ag"))?
            .try_into()
            .unwrap(),
    );
    if version != VERSION {
        return Err(format!(
            "{name}: unsupported .ag version {version:#x} (expected {VERSION:#x})"
        ));
    }
    for start in 12..data.len().saturating_sub(3) {
        if data[start] == 0 || data[start..start + 4].iter().all(|&b| b == 0) {
            continue;
        }
        if let Some(graph) = try_parse(name, data, start) {
            return Ok(graph);
        }
    }
    Err(format!("{name}: no input table found"))
}

fn try_parse(name: &str, data: &[u8], start: usize) -> Option<AnimationGraph> {
    let mut r = Reader::new(data, start);
    let inputs = parse_inputs(&mut r)?;
    if inputs.is_empty() || inputs.len() > 64 {
        return None;
    }

    // Graph header after the factory-node list.
    let factory_objects = r.u32()?;
    if factory_objects != 0 {
        return None;
    }
    r.skip(9 * 4)?;
    r.u16()?;

    let guard_map = r.u32()?;
    if guard_map > 1_000_000 {
        return None;
    }
    for _ in 0..guard_map {
        r.string()?;
        r.u8()?;
    }
    let state_count = r.u32()? as usize;
    if state_count == 0 || state_count > 100_000 {
        return None;
    }
    let mut states = Vec::with_capacity(state_count);
    for id in 0..state_count {
        states.push(parse_state(&mut r, id as u16)?);
    }
    let transition_count = r.u32()? as usize;
    if transition_count > 4_000_000 {
        return None;
    }
    let mut transitions = Vec::with_capacity(transition_count);
    for _ in 0..transition_count {
        let from = r.u16()?;
        let to = r.u16()?;
        let force_follow_chance = r.u16()?;
        let weight = r.u16()?;
        let transition_time = r.f32()?;
        if from as usize >= states.len() || to as usize >= states.len() {
            return None;
        }
        transitions.push(TransitionInfo {
            from,
            to,
            force_follow_chance,
            weight,
            cost_minus1: weight.saturating_sub(states[to as usize].cost_minus1),
            link_cost: weight,
            transition_time,
        });
    }

    // State-name map: state id -> name. It must agree with the state records.
    let name_count = r.u32()? as usize;
    if name_count != states.len() {
        return None;
    }
    let mut names_ok = true;
    for _ in 0..name_count {
        let map_name = r.string()?;
        let id = r.u16()? as usize;
        if id >= states.len() || !states[id].name.eq_ignore_ascii_case(&map_name) {
            names_ok = false;
        }
    }
    if !names_ok {
        return None;
    }
    let state_index = parse_state_index(&mut r)?;

    let serialized_hub = r.u16()?;
    let _flag182 = r.u8()?;

    // The trailing maps are not used by the state machine, but parsing them proves
    // that the section boundaries are right.
    let a = r.u32()?;
    for _ in 0..a {
        r.string()?;
        r.u32()?;
    }
    let b = r.u32()?;
    for _ in 0..b {
        r.string()?;
        r.u32()?;
    }
    r.u32()?;
    r.u32()?;
    let c = r.u32()?;
    for _ in 0..c {
        r.u32()?;
        r.string()?;
        r.string()?;
    }
    r.u8()?;
    let d = r.u32()?;
    for _ in 0..d {
        r.string()?;
        r.u8()?;
    }
    if r.pos != data.len() {
        return None;
    }

    let default_state = states
        .iter()
        .find(|s| {
            s.name.eq_ignore_ascii_case("Main Hub") || s.name.eq_ignore_ascii_case("Main_Hub")
        })
        .map(|s| s.id)
        .or_else(|| {
            (serialized_hub != 0xffff && (serialized_hub as usize) < states.len())
                .then_some(serialized_hub)
        });

    Some(AnimationGraph {
        name: name.to_string(),
        inputs,
        states,
        transitions,
        default_state,
        state_index: Some(state_index),
        source: GraphSource::Ag,
    })
}

fn parse_inputs(r: &mut Reader<'_>) -> Option<Vec<GraphInput>> {
    let count = r.u32()? as usize;
    if count == 0 || count > 64 {
        return None;
    }
    let mut inputs = Vec::with_capacity(count);
    for id in 0..count {
        let kind = match r.u32()? {
            0 => InputKind::Int,
            1 => InputKind::Float,
            2 => InputKind::Keyed,
            _ => return None,
        };
        let _v8 = r.u32()?;
        let name = r.string()?;
        if name.is_empty() {
            return None;
        }
        let _v4 = r.u16()?;
        let signalled = r.u8()? != 0;
        let default_encoded = r.u16()?;
        let _v14 = r.u32()?;
        let priority = r.u8()?;
        let mut input = GraphInput {
            id: id as u16,
            name,
            kind,
            signalled,
            priority,
            force_guard: false,
            mixin_filter: false,
            default_text: None,
            default_encoded,
            min: 0,
            max: 0,
            scale: 0.0,
            offset: 0.0,
            ranges: Vec::new(),
            keys: Vec::new(),
        };
        match kind {
            InputKind::Keyed => {
                let keys = r.u32()? as usize;
                if keys > 65_536 {
                    return None;
                }
                for index in 0..keys {
                    let key = r.string()?;
                    let stored = r.u16()?;
                    if stored as usize != index {
                        return None;
                    }
                    input.keys.push((key, stored));
                }
                let _id = r.u16()?;
            }
            InputKind::Int | InputKind::Float => {
                let range_count = r.u32()? as usize;
                if range_count > 100_000 {
                    return None;
                }
                input.ranges.reserve(range_count);
                for _ in 0..range_count {
                    let min = r.u32()? as i32;
                    let max = r.u32()? as i32;
                    input.ranges.push((min, max));
                }
                input.min = r.u32()? as i32;
                input.max = r.u32()? as i32;
                if kind == InputKind::Float {
                    input.scale = r.f32()?;
                    input.offset = r.f32()?;
                }
                let _tail = r.u16()?;
            }
        }
        inputs.push(input);
    }
    Some(inputs)
}

fn parse_state(r: &mut Reader<'_>, id: u16) -> Option<StateInfo> {
    let name = r.string()?;
    let parent = r.u16()?;
    let _factory_start = r.u32()?;
    let factory_count = r.u8()? as u16;
    let mut serialized_flags = [0u8; 11];
    for f in &mut serialized_flags {
        *f = r.u8()?;
    }
    let _flag8 = r.u8()?;
    let _flag9 = r.u8()?;
    let cost_minus1 = r.u16()?;
    r.skip(4 + 4 + 2)?;

    let mut guards = Vec::new();
    let mut mentioned_inputs = Vec::new();
    let mut count = 0u8;
    for i in 0..16 {
        let input = r.u16()?;
        let value = r.u16()?;
        let packed = ((value & 0x3ff) << 5) | (input & 0x1f) | 0x8000;
        guards.push(packed);
        if value != 0x300 {
            count = (i + 1) as u8;
            if !mentioned_inputs.contains(&(input & 0x1f)) {
                mentioned_inputs.push(input & 0x1f);
            }
        }
    }
    r.skip(12 + 16 + 8)?;
    for (i, guard) in guards.iter_mut().enumerate() {
        if i >= 2 {
            *guard &= 0x7fff;
        }
    }
    guards.truncate(count as usize);

    let mut flags8 = 0u8;
    let mut flags9 = 0u8;
    // The 11 serialized bits are `state+6`/`state+7` minus the `CachePass` flags.
    flags8 |= serialized_flags[0] & 1;
    flags8 |= (serialized_flags[2] & 1) << 1;
    flags8 |= (serialized_flags[3] & 1) << 2;
    flags8 |= (serialized_flags[4] & 1) << 3;
    flags8 |= (serialized_flags[5] & 1) << 4;
    flags8 |= (serialized_flags[6] & 1) << 5;
    flags8 |= (serialized_flags[7] & 1) << 6;
    flags9 |= serialized_flags[8] & 1;
    flags9 |= (serialized_flags[9] & 1) << 1;
    flags9 |= (serialized_flags[10] & 1) << 2;

    Some(StateInfo {
        id,
        name,
        parent: (parent != 0xffff).then_some(parent),
        factory_count,
        cost_minus1,
        flags8,
        flags9,
        flags10: 0,
        guard_count: count,
        select_guard_count: count,
        bare_select: false,
        guards,
        allow_select: serialized_flags[0] & 1 != 0,
        include_in_game: true,
        can_mix: serialized_flags[1] & 1 != 0,
        templates: Vec::new(),
        mentioned_inputs,
    })
}

fn parse_state_index(r: &mut Reader<'_>) -> Option<StateIndex> {
    let entry_count = r.u32()? as usize;
    if entry_count > 1_000_000 {
        return None;
    }
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        entries.push(StateIndexEntry {
            input: r.u16()?,
            value: r.u16()?,
            count: r.u16()?,
            unk6: r.u16()?,
            priority: r.u8()?,
            start: r.u32()?,
            end: r.u32()?,
        });
    }
    let pair_count = r.u32()? as usize;
    if pair_count > 4_000_000 {
        return None;
    }
    let mut pairs = Vec::with_capacity(pair_count);
    for _ in 0..pair_count {
        pairs.push(r.u16()?);
    }
    let input_count = r.u16()?;
    let state_count = r.u16()?;
    let result_count = r.u32()? as usize;
    if result_count > 1_000_000 {
        return None;
    }
    r.skip(result_count.checked_mul(17)?)?;
    Some(StateIndex {
        entries,
        pairs,
        input_count,
        state_count,
    })
}
