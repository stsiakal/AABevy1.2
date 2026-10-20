//! Where addons live and which of them are enabled.
//!
//! The first existing directory of `AA_ADDONS_DIR`, `%LOCALAPPDATA%/AABevy1.2/addons` and
//! `<exe>/addons` is the addons directory. Its `addons.txt` lists the enabled folder names in load
//! order (`#` starts a comment). Every folder holds a `main.lua` that returns the addon's table.

use std::path::{Path, PathBuf};

pub(super) const MAIN_LUA: &str = "main.lua";

const MAX_LIST_BYTES: u64 = 64 * 1024;
const MAX_MAIN_BYTES: u64 = 1024 * 1024;
const MAX_FOLDER_LEN: usize = 64;

/// What `main.lua` returned (the display fields; the callbacks are read from the returned table).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub author: String,
    pub desc: String,
    pub version: String,
}

fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = std::env::var_os("AA_ADDONS_DIR") {
        roots.push(PathBuf::from(dir));
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/local"));
    roots.push(base.join("AABevy1.2/addons"));
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        roots.push(dir.join("addons"));
    }
    roots
}

fn first_existing(roots: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    roots.into_iter().find(|path| path.is_dir())
}

pub(super) fn find_root() -> Option<PathBuf> {
    first_existing(roots())
}

/// A folder name that names a direct child of the addons directory and nothing else.
pub(super) fn valid_folder(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.len() <= MAX_FOLDER_LEN
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && !name.contains("..")
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
        && !name.contains('\0')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '))
}

/// The enabled folders, in `addons.txt` order, without duplicates.
pub(super) fn read_enabled(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join("addons.txt");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if text.len() as u64 > MAX_LIST_BYTES {
        return Err(format!("{} is too large", path.display()));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let name = line.split('#').next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        if !valid_folder(name) {
            bevy::log::warn!("addons: {}:{}: invalid folder name {name:?}", path.display(), line_no + 1);
            continue;
        }
        if seen.insert(name.to_ascii_lowercase()) {
            out.push(name.to_string());
        }
    }
    Ok(out)
}

/// The source of an addon's `main.lua`, which must be inside its own folder.
pub(super) fn read_main(dir: &Path) -> Result<String, String> {
    let path = dir.join(MAIN_LUA);
    let canon_root = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let canon = std::fs::canonicalize(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !canon.starts_with(&canon_root) {
        return Err(format!("{} is outside {}", canon.display(), canon_root.display()));
    }
    let meta = std::fs::metadata(&canon).map_err(|e| format!("{}: {e}", canon.display()))?;
    if !meta.is_file() {
        return Err(format!("{} is not a file", canon.display()));
    }
    if meta.len() > MAX_MAIN_BYTES {
        return Err(format!("{} is too large", canon.display()));
    }
    std::fs::read_to_string(&canon).map_err(|e| format!("{}: {e}", canon.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_cannot_escape_the_addons_directory() {
        for good in ["hello", "my_addon", "addon-2", "Some Addon", "v1.2"] {
            assert!(valid_folder(good), "{good}");
        }
        for bad in ["", " ", ".", "..", "../x", "a/b", "a\\b", "C:addon", "/addon", ".hidden", "a..b", "x\0y"] {
            assert!(!valid_folder(bad), "{bad:?}");
        }
        assert!(!valid_folder(&"a".repeat(MAX_FOLDER_LEN + 1)));
    }

    #[test]
    fn the_first_existing_directory_wins() {
        let base = std::env::temp_dir().join(format!("aa-addons-manifest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let second = base.join("second");
        std::fs::create_dir_all(&second).unwrap();
        let roots = vec![base.join("missing"), second.clone(), base.join("third")];
        assert_eq!(first_existing(roots), Some(second));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn addons_txt_skips_comments_and_duplicates() {
        let base = std::env::temp_dir().join(format!("aa-addons-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(
            base.join("addons.txt"),
            "# the enabled addons\nhello\n\nbad/name # not a folder\nhello\nWorld  \n",
        )
        .unwrap();
        assert_eq!(read_enabled(&base).unwrap(), ["hello", "World"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn main_lua_must_stay_in_its_folder() {
        let base = std::env::temp_dir().join(format!("aa-addons-main-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("hello");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(MAIN_LUA), "return {}").unwrap();
        assert!(read_main(&dir).unwrap().contains("return"));
        std::fs::remove_file(dir.join(MAIN_LUA)).unwrap();
        assert!(read_main(&dir).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
