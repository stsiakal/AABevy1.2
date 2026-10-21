//! Read-only access to the ArcheAge 1.2 `game_pak` archive, optionally with overlay paks.
//!
//! Layout (little endian): `[file data][file table][pad to 0x200][header 0x200]`. The header and
//! every 0x150-byte file table entry are AES-128-CBC encrypted on their own with a zero IV and the
//! XLGames key. File data is stored raw (no compression, no encryption).
//!
//! Paths are matched case-insensitively with `/` or `\` separators. CryEngine paths are relative
//! to the `game` folder (`objects/characters/...`), pak paths are not (`game/objects/...`): both
//! forms resolve.
//!
//! Overlays are ordinary `game_pak`-format files loaded after the base pak. Later overlays win on
//! the same normalised name, so an overlay can replace or add individual files without copying
//! the multi-gigabyte stock pak.
//!
//! ```no_run
//! let pak = aa_pak::Pak::global();
//! let toc = pak.read("scriptsbin/x2ui/loginstage/character_create/toc.g").unwrap();
//! ```

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use aes::Aes128;
use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, KeyInit};

/// Where the 1.2 client lives on this machine; override with the `AA_PAK` environment variable.
pub const DEFAULT_PAK: &str = "D:/AA1.2/game_pak";

const KEY: [u8; 16] = [
    0x32, 0x1F, 0x2A, 0xEE, 0xAA, 0x58, 0x4A, 0xB4, 0x9A, 0x6C, 0x9E, 0x09, 0xD5, 0x9E, 0x9C, 0x6F,
];
const HEADER_SIZE: u64 = 0x200;
const ENTRY_SIZE: usize = 0x150;
const NAME_SIZE: usize = 0x108;

/// Location of one file inside the pak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub offset: u64,
    pub size: u64,
    /// Index into the pak's files: 0 is the base pak, 1.. are overlays in load order.
    pub file: u32,
}

/// An open `game_pak` plus its overlays. Cheap to share: reads use positioned I/O and take `&self`.
pub struct Pak {
    path: PathBuf,
    /// Base pak first, then overlays in priority order.
    files: Vec<File>,
    /// Overlay paths that loaded successfully, in priority order.
    overlays: Vec<PathBuf>,
    /// Normalised (lowercase, `/`) name -> entry.
    entries: HashMap<String, Entry>,
    /// Original names in file-table order (base, then overlay-only names).
    names: Vec<String>,
}

impl Pak {
    /// The process-wide pak (`AA_PAK` or [`DEFAULT_PAK`]). Overlays come from the `overlays`
    /// folder next to it and from `AA_PAK_OVERLAYS` (`;`-separated), in that order. Panics if the
    /// base pak cannot be opened; a missing or invalid overlay is reported and skipped.
    pub fn global() -> &'static Pak {
        static PAK: OnceLock<Pak> = OnceLock::new();
        PAK.get_or_init(|| {
            let path = std::env::var("AA_PAK").unwrap_or_else(|_| DEFAULT_PAK.to_string());
            let overlays = global_overlays(&path);
            Pak::open_with_overlays(&path, overlays)
                .unwrap_or_else(|e| panic!("cannot open game_pak {path}: {e}"))
        })
    }

    pub fn open(path: impl AsRef<Path>) -> io::Result<Pak> {
        Pak::open_with_overlays(path, Vec::<PathBuf>::new())
    }

    /// Open `base` plus `overlays` in priority order: later overlays win on the same normalised
    /// name. Missing or invalid overlays are reported on stderr and skipped; only a bad base pak
    /// makes this return an error.
    pub fn open_with_overlays<I, P>(base: impl AsRef<Path>, overlays: I) -> io::Result<Pak>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let path = base.as_ref().to_path_buf();
        let file = File::open(&path)?;
        let mut pak = Pak {
            path,
            files: Vec::new(),
            overlays: Vec::new(),
            entries: HashMap::new(),
            names: Vec::new(),
        };
        pak.add_file(file)?;

        for overlay in overlays {
            let overlay = overlay.as_ref();
            let file = match File::open(overlay) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("aa-pak: skipping overlay {}: {e}", overlay.display());
                    continue;
                }
            };
            match pak.add_file(file) {
                Ok(count) => {
                    pak.overlays.push(overlay.to_path_buf());
                    eprintln!("aa-pak: overlay {}: {count} files", overlay.display());
                }
                Err(e) => eprintln!("aa-pak: skipping overlay {}: {e}", overlay.display()),
            }
        }
        Ok(pak)
    }

    /// Parse one pak file into `files` and merge its names at the current priority. Returns the
    /// number of file-table entries.
    fn add_file(&mut self, file: File) -> io::Result<usize> {
        let parsed = read_entries(&file)?;
        let count = parsed.len();
        let file_index = self.files.len() as u32;
        self.files.push(file);
        for (name, offset, size) in parsed {
            let key = normalize(&name);
            let entry = Entry { offset, size, file: file_index };
            if self.entries.insert(key, entry).is_none() {
                self.names.push(name);
            }
        }
        Ok(count)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Paths of the overlays that were loaded successfully, in priority order.
    pub fn overlays(&self) -> impl Iterator<Item = &Path> {
        self.overlays.iter().map(PathBuf::as_path)
    }

    /// Number of files.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// All file names as stored (e.g. `game/ui/hud.dds`), in file-table order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// Names whose normalised form starts with the normalised `prefix` (also tried under `game/`).
    pub fn list<'a>(&'a self, prefix: &str) -> impl Iterator<Item = &'a str> + 'a {
        let p = normalize(prefix);
        let g = format!("game/{p}");
        self.names.iter().map(String::as_str).filter(move |n| {
            let n = normalize(n);
            n.starts_with(&p) || n.starts_with(&g)
        })
    }

    pub fn entry(&self, path: &str) -> Option<Entry> {
        let n = normalize(path);
        self.entries.get(&n).or_else(|| self.entries.get(&format!("game/{n}"))).copied()
    }

    pub fn exists(&self, path: &str) -> bool {
        self.entry(path).is_some()
    }

    /// Whole file contents.
    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let e = self.entry(path).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("not in game_pak: {path}"))
        })?;
        let mut buf = vec![0u8; e.size as usize];
        read_exact_at(&self.files[e.file as usize], &mut buf, e.offset)?;
        Ok(buf)
    }

    /// Whole file as text (UTF-8, BOM stripped; invalid bytes replaced).
    pub fn read_string(&self, path: &str) -> io::Result<String> {
        let b = self.read(path)?;
        let b = b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&b);
        Ok(String::from_utf8_lossy(b).into_owned())
    }
}

/// Overlays configured for the process-wide pak: `overlays/*.pak` next to the base pak (sorted
/// by file name), then `AA_PAK_OVERLAYS` entries (`;`-separated) last as the highest priority.
fn global_overlays(base: &str) -> Vec<PathBuf> {
    let env = std::env::var("AA_PAK_OVERLAYS").ok();
    overlays_for(base, env.as_deref())
}

fn overlays_for(base: &str, env: Option<&str>) -> Vec<PathBuf> {
    let mut overlays = Vec::new();
    if let Some(dir) = Path::new(base).parent() {
        let dir = dir.join("overlays");
        match std::fs::read_dir(&dir) {
            Ok(read_dir) => {
                let mut found: Vec<PathBuf> = read_dir
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pak")))
                    .collect();
                found.sort();
                overlays = found;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => eprintln!("aa-pak: cannot read overlay directory {}: {e}", dir.display()),
        }
    }
    if let Some(list) = env {
        overlays.extend(
            list.split(';').map(str::trim).filter(|p| !p.is_empty()).map(PathBuf::from),
        );
    }
    overlays
}

fn read_entries(file: &File) -> io::Result<Vec<(String, u64, u64)>> {
    let len = file.metadata()?.len();
    if len < HEADER_SIZE {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file is too small for a game_pak"));
    }
    let cipher = Aes128::new(GenericArray::from_slice(&KEY));

    let mut header = vec![0u8; HEADER_SIZE as usize];
    read_exact_at(file, &mut header, len - HEADER_SIZE)?;
    decrypt_cbc(&cipher, &mut header);
    let u32_at = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let (kind, files, extra) = if &header[0..4] == b"WIBO" {
        ('A', u32_at(&header, 8), u32_at(&header, 12))
    } else if &header[8..12] == b"IDEJ" {
        ('B', u32_at(&header, 12), u32_at(&header, 0))
    } else if &header[0..4] == b"ZERO" {
        ('F', u32_at(&header, 8), u32_at(&header, 12))
    } else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown game_pak header"));
    };

    let count = files as u64 + extra as u64;
    let table_len = count * ENTRY_SIZE as u64;
    if len < HEADER_SIZE + table_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "game_pak file table does not fit in the file",
        ));
    }
    let mut table_at = len - HEADER_SIZE - table_len;
    table_at -= table_at % HEADER_SIZE;
    let mut table = vec![0u8; count as usize * ENTRY_SIZE];
    read_exact_at(file, &mut table, table_at)?;

    let mut entries = Vec::with_capacity(files as usize);
    for (i, raw) in table.as_chunks_mut::<ENTRY_SIZE>().0.iter_mut().enumerate() {
        let is_file = if kind == 'B' { i >= extra as usize } else { i < files as usize };
        if !is_file {
            continue;
        }
        decrypt_cbc(&cipher, raw);
        let i64_at = |o: usize| i64::from_le_bytes(raw[o..o + 8].try_into().unwrap()) as u64;
        let (name_at, offset, size) = match kind {
            'A' => (0, i64_at(0x108), i64_at(0x110)),
            'B' => (0x20, i64_at(0x18), i64_at(0x130)),
            _ => (8, i64_at(0x110), i64_at(0x118)),
        };
        let name = &raw[name_at..name_at + NAME_SIZE];
        let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(NAME_SIZE)];
        let name: String = name.iter().map(|&b| b as char).collect(); // latin-1
        entries.push((name, offset, size));
    }
    Ok(entries)
}

/// Lowercase, `/` separators, no leading `./` or `/`.
pub fn normalize(path: &str) -> String {
    let p = path.replace('\\', "/").to_ascii_lowercase();
    let p = p.trim_start_matches("./").trim_start_matches('/');
    p.to_string()
}

fn decrypt_cbc(cipher: &Aes128, data: &mut [u8]) {
    let mut prev = [0u8; 16];
    for block in data.as_chunks_mut::<16>().0 {
        let ct = *block;
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
        for (b, p) in block.iter_mut().zip(prev) {
            *b ^= p;
        }
        prev = ct;
    }
}

fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !buf.is_empty() {
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(file, buf, offset)?;
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(file, buf, offset)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[n..];
        offset += n as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_paths() {
        assert_eq!(normalize("Game\\UI\\Hud.dds"), "game/ui/hud.dds");
        assert_eq!(normalize("./objects/a.cgf"), "objects/a.cgf");
    }

    /// Minimal kind-'A' (`WIBO`) writer, enough to exercise `read_entries`.
    fn write_pak(path: &Path, files: &[(&str, &[u8])]) {
        let cipher = Aes128::new(GenericArray::from_slice(&KEY));
        let mut out = Vec::new();
        let mut entries = Vec::new();
        for &(name, data) in files {
            let offset = out.len() as u64;
            out.extend_from_slice(data);
            entries.push((name, offset, data.len() as u64));
        }
        while out.len() % HEADER_SIZE as usize != 0 {
            out.push(0);
        }
        for &(name, offset, size) in &entries {
            let mut raw = [0u8; ENTRY_SIZE];
            raw[..name.len()].copy_from_slice(name.as_bytes());
            raw[0x108..0x110].copy_from_slice(&offset.to_le_bytes());
            raw[0x110..0x118].copy_from_slice(&size.to_le_bytes());
            encrypt_cbc(&cipher, &mut raw);
            out.extend_from_slice(&raw);
        }
        let mut header = [0u8; HEADER_SIZE as usize];
        header[0..4].copy_from_slice(b"WIBO");
        header[8..12].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        encrypt_cbc(&cipher, &mut header);
        out.extend_from_slice(&header);
        std::fs::write(path, out).unwrap();
    }

    fn encrypt_cbc(cipher: &Aes128, data: &mut [u8]) {
        use aes::cipher::BlockEncrypt;

        let mut prev = [0u8; 16];
        for block in data.as_chunks_mut::<16>().0 {
            for (b, p) in block.iter_mut().zip(prev) {
                *b ^= p;
            }
            cipher.encrypt_block(GenericArray::from_mut_slice(block));
            prev.copy_from_slice(block);
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aa-pak-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn overlays_replace_and_add_files() {
        let dir = temp_dir("overlays");
        let base = dir.join("base.pak");
        let overlay1 = dir.join("overlay1.pak");
        let overlay2 = dir.join("overlay2.pak");
        write_pak(&base, &[("a.txt", b"1"), ("b.txt", b"1")]);
        write_pak(&overlay1, &[("b.txt", b"2"), ("c.txt", b"2")]);
        write_pak(&overlay2, &[("c.txt", b"3")]);

        let pak = Pak::open_with_overlays(&base, [&overlay1, &overlay2]).unwrap();
        assert_eq!(pak.path(), base.as_path());
        assert_eq!(pak.read("a.txt").unwrap(), b"1");
        assert_eq!(pak.read("b.txt").unwrap(), b"2");
        assert_eq!(pak.read("c.txt").unwrap(), b"3");
        assert_eq!(pak.read("B.TXT").unwrap(), b"2");
        assert_eq!(pak.read_string("c.TxT").unwrap(), "3");
        assert!(pak.exists("A.txt") && pak.exists("C.txt") && !pak.exists("d.txt"));
        assert_eq!(pak.names().collect::<Vec<_>>(), ["a.txt", "b.txt", "c.txt"]);
        assert_eq!(pak.names().count(), pak.len());
        assert_eq!(pak.list("").collect::<Vec<_>>(), ["a.txt", "b.txt", "c.txt"]);
        assert_eq!(pak.overlays().collect::<Vec<_>>(), [overlay1.as_path(), overlay2.as_path()]);

        let corrupt = dir.join("corrupt.pak");
        std::fs::write(&corrupt, b"not a pak").unwrap();
        assert!(Pak::open(&corrupt).is_err());
        let pak = Pak::open_with_overlays(&base, [&overlay1, &corrupt]).unwrap();
        assert_eq!(pak.read("b.txt").unwrap(), b"2");
        assert_eq!(pak.read("c.txt").unwrap(), b"2");
        assert_eq!(pak.overlays().collect::<Vec<_>>(), [overlay1.as_path()]);

        let missing = dir.join("missing.pak");
        let pak = Pak::open_with_overlays(&base, [&missing]).unwrap();
        assert_eq!(pak.overlays().count(), 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn overlay_discovery_orders_folder_then_env() {
        let dir = temp_dir("discovery");
        let base = dir.join("game_pak");
        std::fs::write(&base, b"").unwrap();
        let folder = dir.join("overlays");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("b.pak"), b"").unwrap();
        std::fs::write(folder.join("a.PAK"), b"").unwrap();
        std::fs::write(folder.join("skip.txt"), b"").unwrap();

        let z = dir.join("z.pak");
        let y = dir.join("y.pak");
        let env = format!("{}; {} ;;", z.display(), y.display());
        let base = base.to_str().unwrap();
        assert_eq!(
            overlays_for(base, Some(&env)),
            [folder.join("a.PAK"), folder.join("b.pak"), z, y]
        );
        assert_eq!(overlays_for(base, None), [folder.join("a.PAK"), folder.join("b.pak")]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Needs the real client (`AA_PAK` or `D:/AA1.2/game_pak`).
    #[test]
    #[ignore]
    fn reads_real_pak() {
        let pak = Pak::global();
        assert_eq!(pak.len(), 218_066);
        let toc = pak.read_string("scriptsbin/x2ui/loginstage/character_create/toc.g").unwrap();
        assert!(toc.starts_with("## author"), "{}", &toc[..40]);
        let alb = pak.read("GAME\\SCRIPTSBIN\\X2UI\\LOGINSTAGE\\BASELIB.ALB").unwrap();
        assert!(alb.starts_with(b"\x1bLuaQ"));
        assert!(pak.list("ui/login_stage/").count() > 100);
    }
}
