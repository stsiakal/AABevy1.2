//! Per-character UI data: the native option tree (type 1) and key-binding records (type 2).
//!
//! `OnResponseUIData` copies types 1 and 2 verbatim into the per-character buffers, and `Data::SaveToBuffer` writes them back
//! (`0x391e4320`, `0x39217640`), so the blobs are native `Data` trees. Only the key/value pairs this client owns are encoded.
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use aa_net::{
    UiDataCommand, UiDataResponse,
    ui_data::{NativeData, NativeDataNode, MAX_UI_DATA_BYTES},
};
use serde_json::{Value as Json, json};

pub(super) const OPTION_TYPE: u16 = 1;
pub(super) const BINDING_TYPE: u16 = 2;

#[derive(Default)]
struct Dirty {
    owner: u32,
    options: bool,
    bindings: bool,
    since: Option<Instant>,
}

fn dirty() -> &'static Mutex<Dirty> {
    static DIRTY: OnceLock<Mutex<Dirty>> = OnceLock::new();
    DIRTY.get_or_init(|| Mutex::new(Dirty::default()))
}

fn lock() -> std::sync::MutexGuard<'static, Dirty> {
    dirty().lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn mark(owner: u32, options: bool, bindings: bool) {
    let mut d = lock();
    if d.owner != owner {
        *d = Dirty::default();
        d.owner = owner;
    }
    d.options |= options;
    d.bindings |= bindings;
    d.since = Some(Instant::now());
}

pub(super) fn mark_options(owner: u32) {
    mark(owner, true, false);
}

pub(super) fn mark_bindings(owner: u32) {
    mark(owner, false, true);
}

pub(super) fn dirty_types(owner: u32) -> (bool, bool) {
    let d = lock();
    if d.owner == owner {
        (d.options, d.bindings)
    } else {
        (false, false)
    }
}

pub(super) fn dirty_due(owner: u32, window: Duration) -> bool {
    let d = lock();
    if d.owner != owner || (!d.options && !d.bindings) {
        return false;
    }
    d.since.is_none_or(|since| since.elapsed() >= window)
}

pub(super) fn clear(owner: u32) {
    let mut d = lock();
    if d.owner == owner {
        *d = Dirty::default();
    }
}

pub(super) fn requests(character_id: u32) -> [UiDataCommand; 2] {
    [OPTION_TYPE, BINDING_TYPE].map(|data_type| UiDataCommand::Request {
        character_id,
        data_type,
    })
}

fn leaf(value: &[u8]) -> NativeDataNode {
    NativeDataNode {
        value: value.to_vec(),
        children: Vec::new(),
    }
}

fn node(value: &[u8], children: Vec<NativeDataNode>) -> NativeDataNode {
    NativeDataNode {
        value: value.to_vec(),
        children,
    }
}

fn find<'a>(tree: &'a NativeData, name: &[u8]) -> Option<&'a NativeDataNode> {
    tree.children.iter().find(|node| node.value == name)
}

fn encoded(tree: &NativeData) -> Option<String> {
    let bytes = tree.encode().ok()?;
    if bytes.len() > MAX_UI_DATA_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn option_node(key: &str, value: &Json) -> NativeDataNode {
    let value = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
    node(key.as_bytes(), vec![leaf(value.as_bytes())])
}

/// The type-1 `Data` tree: `version 1` and one child of `option` per store key (`n:55`, `s:ShowFps`, ...).
pub(super) fn options_payload(options: Option<&Json>) -> Option<String> {
    let mut root = NativeData::default();
    root.children.push(node(b"version", vec![leaf(b"1")]));
    let mut option = node(b"option", Vec::new());
    if let Some(map) = options.and_then(Json::as_object) {
        for (key, value) in map {
            if key.len() > 256 || serde_json::to_string(value).is_ok_and(|v| v.len() > 4096) {
                continue;
            }
            option.children.push(option_node(key, value));
        }
    }
    root.children.push(option);
    while encoded(&root).is_none() {
        root.children.get_mut(1)?.children.pop()?;
    }
    encoded(&root)
}

fn binding_record(record: &Json) -> Option<NativeDataNode> {
    let text = |name: &str| record.get(name).and_then(Json::as_str).map(str::to_owned);
    let number = |name: &str| record.get(name).and_then(Json::as_u64);
    let manager = number("manager")?;
    let index = number("index")?;
    let kind = number("kind")?;
    let action = text("action")?;
    let mode = text("mode").unwrap_or_else(|| "normal_mode".into());
    let key = text("key").unwrap_or_default();
    Some(node(
        b"binding",
        vec![
            leaf(manager.to_string().as_bytes()),
            leaf(mode.as_bytes()),
            leaf(action.as_bytes()),
            leaf(index.to_string().as_bytes()),
            leaf(kind.to_string().as_bytes()),
            leaf(key.as_bytes()),
        ],
    ))
}

/// The type-2 `Data` tree: `version 1` and one `binding` node per record (manager, mode, action, index, kind, key).
pub(super) fn bindings_payload(bindings: Option<&Json>) -> Option<String> {
    let mut root = NativeData::default();
    root.children.push(node(b"version", vec![leaf(b"1")]));
    let mut binding = node(b"binding", Vec::new());
    if let Some(records) = bindings.filter(|b| b["version"] == 2).and_then(|b| b["records"].as_array()) {
        for record in records.iter().take(4096) {
            if let Some(record) = binding_record(record) {
                binding.children.push(record);
            }
        }
    }
    root.children.push(binding);
    while encoded(&root).is_none() {
        root.children.get_mut(1)?.children.pop()?;
    }
    encoded(&root)
}

fn text(value: &[u8]) -> Option<String> {
    String::from_utf8(value.to_vec()).ok()
}

fn parse_options(tree: &NativeData) -> Option<Json> {
    let option = find(tree, b"option")?;
    let mut map = serde_json::Map::new();
    for child in &option.children {
        let key = text(&child.value)?;
        if key.is_empty() || key.len() > 256 {
            continue;
        }
        let Some(value) = child.children.first().and_then(|value| text(&value.value)) else {
            continue;
        };
        let value = serde_json::from_str(&value).unwrap_or(Json::String(value));
        map.insert(key, value);
    }
    Some(Json::Object(map))
}

fn parse_bindings(tree: &NativeData) -> Option<Json> {
    let binding = find(tree, b"binding")?;
    let mut records = Vec::new();
    for child in &binding.children {
        if child.children.len() != 6 {
            continue;
        }
        let fields: Vec<_> = child
            .children
            .iter()
            .map(|field| text(&field.value))
            .collect();
        let (Some(manager), Some(_mode), Some(action), Some(index), Some(kind), Some(key)) =
            (fields[0].clone(), fields[1].clone(), fields[2].clone(), fields[3].clone(), fields[4].clone(), fields[5].clone())
        else {
            continue;
        };
        let (Ok(manager), Ok(index), Ok(kind)) = (
            manager.parse::<u8>(),
            index.parse::<u32>(),
            kind.parse::<u8>(),
        ) else {
            continue;
        };
        if !(1..=2).contains(&manager) || !matches!(kind, 0 | 2 | 5) || action.len() > 256 {
            continue;
        }
        records.push(json!({
            "manager": manager,
            "mode": _mode,
            "action": action,
            "index": index,
            "kind": kind,
            "key": key,
        }));
    }
    Some(json!({"version": 2, "records": records}))
}

/// A type-1/2 response as the store's JSON shape. Opaque, truncated or unsupported payloads are refused and leave the local cache alone.
pub(super) fn decode(response: &UiDataResponse) -> Option<Json> {
    if response.character_id == 0 || !matches!(response.data_type, OPTION_TYPE | BINDING_TYPE) {
        return None;
    }
    let bytes = response.payload.as_bytes();
    if response.raw_length <= 0 || bytes.is_empty() {
        return None;
    }
    let length = response.raw_length as usize;
    if length != bytes.len() && length != bytes.len() + 1 {
        bevy::log::warn!(
            "settings: refusing truncated or oversized type{} UI data",
            response.data_type
        );
        return None;
    }
    let tree = match NativeData::parse(bytes) {
        Ok(tree) => tree,
        Err(error) => {
            bevy::log::warn!("settings: retaining unsupported type{} UI data: {error:?}", response.data_type);
            return None;
        }
    };
    match response.data_type {
        OPTION_TYPE => parse_options(&tree),
        BINDING_TYPE => parse_bindings(&tree),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_trees_round_trip_options_and_bindings() {
        let options = json!({"n:42": 1, "s:ShowFps": 0, "s:Name": "Some value"});
        let payload = options_payload(Some(&options)).unwrap();
        let tree = NativeData::parse(payload.as_bytes()).unwrap();
        assert_eq!(parse_options(&tree), Some(options));

        let bindings = json!({"version": 2, "records": [
            {"manager": 1, "mode": "normal_mode", "action": "moveleft", "index": 0, "kind": 0, "key": "q"},
            {"manager": 2, "mode": "normal_mode", "action": "action_bar_button", "index": 3, "kind": 5, "key": "shift-1"}
        ]});
        let payload = bindings_payload(Some(&bindings)).unwrap();
        let tree = NativeData::parse(payload.as_bytes()).unwrap();
        assert_eq!(parse_bindings(&tree), Some(bindings));
    }

    #[test]
    fn empty_and_invalid_trees_are_refused_or_empty() {
        let empty = NativeData::default();
        assert_eq!(parse_options(&empty), None);
        assert_eq!(parse_bindings(&empty), None);
        let payload = "version 1\r\noption\r\n";
        let response = UiDataResponse {
            character_id: 7,
            data_type: OPTION_TYPE,
            payload: payload.into(),
            raw_length: payload.len() as i32 + 1,
        };
        assert_eq!(decode(&response), Some(json!({})));
        let truncated = UiDataResponse { raw_length: 99, ..response };
        assert_eq!(decode(&truncated), None);
    }

    #[test]
    fn dirty_window_is_per_character_and_can_be_taken() {
        clear(7);
        mark_options(7);
        assert_eq!(dirty_types(7), (true, false));
        assert!(!dirty_due(7, Duration::from_secs(60)));
        mark_bindings(7);
        assert_eq!(dirty_types(7), (true, true));
        clear(7);
        assert_eq!(dirty_types(7), (false, false));
        assert_eq!(requests(7).len(), 2);
    }
}
