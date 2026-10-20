//! Per-addon settings: one JSON file per addon under
//! `%LOCALAPPDATA%/AABevy1.2/addons/settings/<folder>.json`.
//!
//! Values are written with the same limits the loader accepts on read: 16 levels deep, 4096 keys
//! and 64 KB strings. Writes are atomic (temporary file, then rename), like `settings.rs`.

use std::path::{Path, PathBuf};

use mlua::{Lua, Value};
use serde_json::{Map, Value as Json};

pub(super) const MAX_DEPTH: usize = 16;
pub(super) const MAX_KEYS: usize = 4096;
pub(super) const MAX_STRING: usize = 64 * 1024;
const MAX_FILE: u64 = 4 * 1024 * 1024;

pub(super) fn settings_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/local"));
    base.join("AABevy1.2/addons/settings")
}

pub(super) fn load(path: &Path) -> Result<Json, String> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Json::Object(Map::new()));
        }
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if meta.len() > MAX_FILE {
        return Err(format!("{} is too large", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: Json = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if !json.is_object() && !json.is_array() {
        return Err(format!("{}: the root must be a table", path.display()));
    }
    let mut keys = 0;
    validate(&json, 1, &mut keys)?;
    Ok(json)
}

pub(super) fn save(path: &Path, json: &Json) -> Result<(), String> {
    let mut keys = 0;
    validate(json, 1, &mut keys)?;
    let bytes = serde_json::to_vec_pretty(json).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(format!("{}: settings are too large", path.display()));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let temp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&temp, &bytes).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn validate(json: &Json, depth: usize, keys: &mut usize) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("settings are nested deeper than {MAX_DEPTH} levels"));
    }
    match json {
        Json::Null | Json::Bool(_) => Ok(()),
        Json::Number(number) => {
            if number.as_f64().is_none_or(|n| !n.is_finite()) {
                return Err("settings hold a non-finite number".into());
            }
            Ok(())
        }
        Json::String(text) => {
            if text.len() > MAX_STRING {
                return Err(format!("a settings string is longer than {MAX_STRING} bytes"));
            }
            Ok(())
        }
        Json::Array(items) => {
            for item in items {
                *keys += 1;
                if *keys > MAX_KEYS {
                    return Err(format!("settings hold more than {MAX_KEYS} keys"));
                }
                validate(item, depth + 1, keys)?;
            }
            Ok(())
        }
        Json::Object(map) => {
            for (key, item) in map {
                *keys += 1;
                if *keys > MAX_KEYS {
                    return Err(format!("settings hold more than {MAX_KEYS} keys"));
                }
                if key.len() > MAX_STRING {
                    return Err(format!("a settings key is longer than {MAX_STRING} bytes"));
                }
                validate(item, depth + 1, keys)?;
            }
            Ok(())
        }
    }
}

fn string_of(value: &mlua::LuaString) -> Result<String, String> {
    let text = value.to_str().map_err(|_| "settings strings must be UTF-8")?;
    if text.len() > MAX_STRING {
        return Err(format!("a settings string is longer than {MAX_STRING} bytes"));
    }
    Ok(text.to_string())
}

pub(super) fn to_json(value: &Value) -> Result<Json, String> {
    let mut keys = 0;
    encode(value, 1, &mut keys)
}

fn encode(value: &Value, depth: usize, keys: &mut usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err(format!("settings are nested deeper than {MAX_DEPTH} levels"));
    }
    match value {
        Value::Nil => Ok(Json::Null),
        Value::Boolean(value) => Ok(Json::Bool(*value)),
        Value::Integer(number) => Ok(Json::from(*number)),
        Value::Number(number) => serde_json::Number::from_f64(*number)
            .map(Json::Number)
            .ok_or_else(|| "settings hold a non-finite number".to_string()),
        Value::String(text) => Ok(Json::String(string_of(text)?)),
        Value::Table(table) => {
            let entries: Vec<(Value, Value)> = table
                .clone()
                .pairs::<Value, Value>()
                .collect::<mlua::Result<_>>()
                .map_err(|e| e.to_string())?;
            let len = entries.len();
            let mut is_array = len > 0;
            let mut seen = vec![false; len];
            for (key, _) in &entries {
                match key {
                    Value::Integer(index) if *index >= 1 && (*index as usize) <= len && !seen[*index as usize - 1] => {
                        seen[*index as usize - 1] = true;
                    }
                    _ => {
                        is_array = false;
                        break;
                    }
                }
            }
            if is_array {
                let mut items: Vec<Option<Json>> = vec![None; len];
                for (key, item) in &entries {
                    *keys += 1;
                    if *keys > MAX_KEYS {
                        return Err(format!("settings hold more than {MAX_KEYS} keys"));
                    }
                    let Value::Integer(index) = key else { unreachable!() };
                    items[*index as usize - 1] = Some(encode(item, depth + 1, keys)?);
                }
                return Ok(Json::Array(items.into_iter().map(|item| item.unwrap_or(Json::Null)).collect()));
            }
            let mut map = Map::new();
            for (key, item) in entries {
                *keys += 1;
                if *keys > MAX_KEYS {
                    return Err(format!("settings hold more than {MAX_KEYS} keys"));
                }
                let key = match key {
                    Value::String(text) => {
                        let text = string_of(&text)?;
                        if text.starts_with('#') { format!("#s:{text}") } else { text }
                    }
                    Value::Integer(index) => format!("#i:{index}"),
                    _ => return Err("settings tables can only have string or integer keys".into()),
                };
                map.insert(key, encode(&item, depth + 1, keys)?);
            }
            Ok(Json::Object(map))
        }
        _ => Err("settings can only hold numbers, strings, booleans and tables".into()),
    }
}

pub(super) fn to_lua(lua: &Lua, json: &Json) -> Result<Value, String> {
    let mut keys = 0;
    decode(lua, json, 1, &mut keys)
}

fn decode(lua: &Lua, json: &Json, depth: usize, keys: &mut usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err(format!("settings are nested deeper than {MAX_DEPTH} levels"));
    }
    Ok(match json {
        Json::Null => Value::Nil,
        Json::Bool(value) => Value::Boolean(*value),
        Json::Number(number) => match number.as_i64() {
            Some(value) => Value::Integer(value),
            None => Value::Number(number.as_f64().unwrap_or_default()),
        },
        Json::String(text) => {
            if text.len() > MAX_STRING {
                return Err(format!("a settings string is longer than {MAX_STRING} bytes"));
            }
            Value::String(lua.create_string(text).map_err(|e| e.to_string())?)
        }
        Json::Array(items) => {
            let table = lua.create_table().map_err(|e| e.to_string())?;
            for (index, item) in items.iter().enumerate() {
                *keys += 1;
                if *keys > MAX_KEYS {
                    return Err(format!("settings hold more than {MAX_KEYS} keys"));
                }
                table
                    .raw_set(index + 1, decode(lua, item, depth + 1, keys)?)
                    .map_err(|e| e.to_string())?;
            }
            Value::Table(table)
        }
        Json::Object(map) => {
            let table = lua.create_table().map_err(|e| e.to_string())?;
            for (key, item) in map {
                *keys += 1;
                if *keys > MAX_KEYS {
                    return Err(format!("settings hold more than {MAX_KEYS} keys"));
                }
                if key.len() > MAX_STRING {
                    return Err(format!("a settings key is longer than {MAX_STRING} bytes"));
                }
                let key = if let Some(number) = key.strip_prefix("#i:") {
                    let number = number
                        .parse::<i64>()
                        .map_err(|_| format!("invalid settings key {key:?}"))?;
                    Value::Integer(number)
                } else if let Some(text) = key.strip_prefix("#s:") {
                    Value::String(lua.create_string(text).map_err(|e| e.to_string())?)
                } else {
                    Value::String(lua.create_string(key).map_err(|e| e.to_string())?)
                };
                table
                    .raw_set(key, decode(lua, item, depth + 1, keys)?)
                    .map_err(|e| e.to_string())?;
            }
            Value::Table(table)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_tables_arrays_and_numeric_keys() {
        let lua = aa_lua::new_lua().unwrap();
        let table = lua
            .load(
                "return {name='hello', nested={1, 2, 3}, ['with space']=true, ['#tag']='hash', [7]='seven', empty={}}",
            )
            .eval::<Value>()
            .unwrap();
        let json = to_json(&table).unwrap();
        let back = to_lua(&lua, &json).unwrap();
        let check = lua
            .load(
                "return function(t) return t.name=='hello' and #t.nested==3 and t.nested[3]==3 and t['with space']==true \
                 and t['#tag']=='hash' and t[7]=='seven' and type(t.empty)=='table' end",
            )
            .eval::<mlua::Function>()
            .unwrap();
        assert!(check.call::<bool>(back).unwrap());
    }

    #[test]
    fn files_round_trip_and_missing_is_empty() {
        let base = std::env::temp_dir().join(format!("aa-addons-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("hello.json");
        assert_eq!(load(&path).unwrap(), json!({}));
        let value = json!({"a": 1, "b": [true, "x"]});
        save(&path, &value).unwrap();
        assert_eq!(load(&path).unwrap(), value);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn limits_are_enforced() {
        let mut deep = json!(1);
        for _ in 0..MAX_DEPTH {
            deep = json!([deep]);
        }
        let mut keys = 0;
        assert!(validate(&deep, 1, &mut keys).is_err());

        let mut map = Map::new();
        for i in 0..=MAX_KEYS {
            map.insert(format!("k{i}"), json!(1));
        }
        let mut keys = 0;
        assert!(validate(&Json::Object(map), 1, &mut keys).is_err());

        let mut keys = 0;
        assert!(validate(&Json::String("x".repeat(MAX_STRING + 1)), 1, &mut keys).is_err());
    }

    #[test]
    fn unsupported_lua_values_are_rejected() {
        let lua = aa_lua::new_lua().unwrap();
        let value = lua.load("return {fn=function() end}").eval::<Value>().unwrap();
        assert!(to_json(&value).is_err());
    }
}
