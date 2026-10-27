#!/usr/bin/env python3
"""Audit the proof level of every ported 1.2 function address.

The audit is deliberately a set of readable heuristics, not a Rust parser: it
walks the ported sources, collects the function addresses named in doc
comments and `fun_xxxxxxxx` identifiers, finds the goldens and the tests that
load them, and classifies how the port is proven and whether the game calls it.

Levels
------
EXACT       a live golden is compared bit-for-bit (f32/f64 bits) or byte-for-byte
TOLERANCE   a live golden is compared within a tolerance (the tolerance is recorded)
DECOMPILE   ported from the decompile, no live golden (unit tests only, or none)
APPROX      an approximate port / stand-in: measured rule, guessed constant, Bevy stand-in
REFERENCED  the address is only named in a comment; no Rust implementation was found

`+UNWIRED` is appended when no code under `ingame/` or `hud/` reaches the
function (directly, through a used port type, or through the port call graph).

Rows the heuristics cannot settle are written to the review file instead of
being silently trusted. Run from the repository root:

    python tools/port_audit/proof_levels.py

Outputs (default `docs/research/`):
    proof-levels.csv          one row per address
    proof-levels-table.md     the same rows as a markdown table
    proof-levels-review.md    rows that need a human eye, with the reason
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import re
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path


REPO = Path(__file__).resolve().parents[2]
PORT = REPO / "crates" / "aa-client" / "src" / "port"
PHYSICS = REPO / "crates" / "aa-physics"
INGAME = REPO / "crates" / "aa-client" / "src" / "ingame"
HUD = REPO / "crates" / "aa-client" / "src" / "hud"

SLICE_INFO = {
    "anim": ("F3/F4", "animation graph and mount flow"),
    "camera": ("F2", "third-person camera"),
    "combat": ("S1", "client combat"),
    "combat_anim": ("E6", "combat animation"),
    "cryanim": ("E2", "CryAnimation playback"),
    "formulas": ("S8", "formulas and stats"),
    "housing": ("S4", "housing and placement"),
    "interaction": ("S2", "doodad and NPC interaction"),
    "movement": ("F1/S5", "movement and movement extras"),
    "remote": ("F7", "remote-unit smoothing"),
    "vehicles": ("S3", "vehicles and ships"),
    "world": ("E9/world", "world queries"),
    "physics": ("F6", "aa-physics"),
}
SLICE_ORDER = list(SLICE_INFO) + [""]

LEVEL_ORDER = {"EXACT": 0, "TOLERANCE": 1, "DECOMPILE": 2, "APPROX": 3, "REFERENCED": 4, "GOLDEN-ONLY": 5}

FUN_ADDR_RE = re.compile(r"(?i)\bFUN_([0-9a-f]{8})\b")
FUN_IDENT_RE = re.compile(r"(?i)\bfun_([0-9a-f]{8})\b")
BARE_ADDR_RE = re.compile(r"(?i)(?<![0-9a-z_])([0-9a-f]{8})(?![0-9a-z_])")
ANY_ADDR_RE = re.compile(r"(?i)(?<![0-9a-f])(?:0x)?([0-9a-f]{8})(?![0-9a-f])")
FN_RE = re.compile(
    r"^\s*(?P<pub>pub(?:\([^)]*\))?\s+)?"
    r"(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?"
    r"fn\s+(?P<name>[A-Za-z_]\w*)"
)
IMPL_RE = re.compile(r"^\s*impl(?:<[^>]*>)?\s+(?:[A-Za-z_]\w*\s+for\s+)?(?P<type>[A-Za-z_]\w*)")
TYPE_RE = re.compile(r"^\s*pub\s+(?:struct|enum|type)\s+(?P<type>[A-Za-z_]\w*)")
TEST_RE = re.compile(r"^\s*#\[test\]\s*$")
COMMENT_ONLY = re.compile(r"^\s*//")
INCLUDE_RE = re.compile(r'include_str!\(\s*"([^"]+\.json)"\s*\)')
LOAD_ARM_RE = re.compile(r'"([^"]+)"\s*=>\s*include_str!\(\s*"([^"]+\.json)"\s*\)')
LOAD_CALL_RE = re.compile(r'\b(?:load|load_live|golden|json|doc|goldens)\s*\(\s*"([^"]+)"\s*\)')

APPROX_MARKERS = (
    "stand-in",
    "stand in",
    "hand-fitted",
    "hand fitted",
    "hand-tuned",
    "hand tuned",
    "guessed",
    "guess the",
    "measured rule",
    "measured constant",
    "approximate",
    "approximation of",
    "imitates",
    "imitating",
    "bevy stand-in",
    "not the original",
)
DECOMPILE_MARKERS = (
    "decompile only",
    "decompile-only",
    "unit tests only",
    "no live",
    "without live",
    "not live",
    "synthetic",
    "crafted",
    "cannot be called",
    "blocked",
    "no live golden",
    "never fired",
)
COMMON_METHODS = {"new", "default", "from", "update", "get", "set", "len", "is_empty", "drop"}

# Deliberate stand-ins that a live golden cannot make exact. Keep this list
# short and cite the source comment that says why.
LEVEL_OVERRIDES = {
    "390a30e0": (
        "APPROX",
        "mode 3 follows the extrapolated sample with a stand-in response; the actor-model queue (FUN_390ad070/FUN_390b2cf0) is not ported (port/remote/mod.rs)",
    ),
    "390a3bb0": (
        "APPROX",
        "Update runs the mode-3 stand-in path (port/remote/mod.rs)",
    ),
    "393989e0": (
        "APPROX",
        "the request body is a caller-supplied predicate standing in for the un-ported split tail (port/combat/front.rs)",
    ),
}


@dataclass
class Func:
    path: Path
    rel: str
    name: str
    line: int
    doc: str
    body: str
    impl_type: str | None
    is_pub: bool
    slice: str
    addresses: set[str] = field(default_factory=set)

    @property
    def id(self) -> str:
        return f"{self.rel}:{self.line}:{self.name}"


@dataclass
class Golden:
    rel: str
    slice: str
    addresses: set[str] = field(default_factory=set)
    meta: list[str] = field(default_factory=list)
    source: str = ""
    live: str = "unknown"
    synthetic: bool = False


@dataclass
class TestInfo:
    path: Path
    rel: str
    name: str
    line: int
    body: str
    goldens: set[str] = field(default_factory=set)
    kind: str = "UNKNOWN"
    tolerance: list[str] = field(default_factory=list)
    exact: bool = False
    notes: list[str] = field(default_factory=list)


def rel(path: Path) -> str:
    return path.relative_to(REPO).as_posix()


def slice_of(path: Path) -> str:
    try:
        first = path.resolve().relative_to(PORT).parts[0]
    except ValueError:
        posix = path.as_posix()
        marker = "crates/aa-client/src/port/"
        if marker in posix:
            first = posix.split(marker, 1)[1].split("/", 1)[0]
        else:
            return "physics" if "aa-physics" in posix else ""
    return first if first in SLICE_INFO else ""


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def line_comment(line: str) -> str | None:
    idx = line.find("//")
    return line[idx:] if idx >= 0 else None


def extract_comment_addresses(text: str) -> set[str]:
    """Addresses in a comment or doc string, including the bare `3155c710` form."""
    found: set[str] = set()
    for m in FUN_ADDR_RE.finditer(text):
        found.add(m.group(1).lower())
    for m in FUN_IDENT_RE.finditer(text):
        found.add(m.group(1).lower())
    for m in BARE_ADDR_RE.finditer(text):
        token = m.group(1).lower()
        if token == "00000000" or token in found:
            continue
        before = text[max(0, m.start() - 2) : m.start()].lower()
        after = text[m.end() : m.end() + 2]
        if before.endswith("0x") or after.startswith("..") or before.endswith(".."):
            continue
        ticked = (
            m.start() > 0
            and text[m.start() - 1] == "`"
            and m.end() < len(text)
            and text[m.end()] == "`"
        )
        window = text[max(0, m.start() - 48) : m.end() + 48].lower()
        function_context = any(
            word in window
            for word in ("address", "function", "fun_", "fn ", "rva", "call", "code ")
        )
        if ticked or function_context:
            found.add(token)
    return found


CALLER_WORDS = (
    "called from",
    "caller",
    "calls ",
    "call to",
    "invoked",
    "invokes",
    "dispatched from",
    "reached from",
)


def implementation_addresses(text: str) -> set[str]:
    """The addresses that name the function being documented.

    A function doc opens with its own address (sometimes two, e.g.
    ``FUN_3933fd00 + FUN_3933ac10`` for a struct writer/reader pair) and then,
    after a blank line, cites callers and neighbouring functions. Only the
    first paragraph is the implementation; later paragraphs are cross-refs.

    A word like "caller" inside the first paragraph ("the queries are supplied
    by the caller") describes the implementation, so only lines that *start*
    with a caller phrase are treated as a cross-reference.
    """
    paragraph: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped:
            if paragraph:
                break
            continue
        if stripped.startswith("///") or stripped.startswith("//!"):
            paragraph.append(stripped)
        elif paragraph:
            break
    if not paragraph:
        paragraph = [ln.strip() for ln in text.splitlines() if ln.strip()]
    found: set[str] = set()
    for line in paragraph:
        addrs = extract_comment_addresses(line)
        if not addrs:
            continue
        low = re.sub(r"^(///+|//!)\s*[*`]?", "", line).strip().lower()
        if any(low.startswith(word) for word in CALLER_WORDS):
            continue
        if re.match(r"(///+|//!)\s*[*`]?\s*(FUN_|Address\b|0x)", line, re.I):
            found |= addrs
        elif not found:
            found |= addrs
    return found


def extract_field_addresses(text: str, key: str = "") -> set[str]:
    """Addresses in golden metadata (`function`, `source`, ...), including bare ones.

    A note can contain decimal ids that look like eight hex digits (`item_id:
    16777227`); outside the explicit `function`/`address` fields a token must
    carry a hex letter or a `0x` prefix. Image bases (`...000000`) and the words
    "image base" are not function addresses.
    """
    found: set[str] = set()
    if len(text) > 600:
        return found
    if re.fullmatch(r"[0-9a-fA-F]+", text.strip()) and len(text.strip()) > 16:
        return found
    for m in ANY_ADDR_RE.finditer(text):
        token = m.group(1).lower()
        raw = m.group(0).lower()
        if token == "00000000":
            continue
        if key not in {"function", "address"}:
            if not any(c in "abcdef" for c in token) and not raw.startswith("0x"):
                continue
            before = text[max(0, m.start() - 24) : m.start()].lower()
            if "base" in before:
                continue
        if token.endswith("000000"):
            continue
        found.add(token)
    return found


def brace_block(lines: list[str], start: int) -> int:
    """Return the last line index of the brace block that starts at/after `start`."""
    depth = 0
    seen = False
    for i in range(start, len(lines)):
        code = strip_comments(lines[i])
        for ch in code:
            if ch == "{":
                depth += 1
                seen = True
            elif ch == "}":
                depth -= 1
                if seen and depth <= 0:
                    return i
    return len(lines) - 1


def test_regions(lines: list[str]) -> list[tuple[int, int]]:
    regions: list[tuple[int, int]] = []
    for i, line in enumerate(lines):
        if re.match(r"^\s*#\[cfg\(test\)\]", line):
            j = i + 1
            while j < len(lines) and not re.match(r"^\s*mod\s+\w+", lines[j]):
                j += 1
            if j < len(lines):
                regions.append((i, brace_block(lines, j)))
    return regions


def inside(regions: list[tuple[int, int]], line: int) -> bool:
    return any(a <= line <= b for a, b in regions)


def doc_above(lines: list[str], idx: int) -> str:
    out: list[str] = []
    j = idx - 1
    while j >= 0:
        stripped = lines[j].strip()
        if stripped.startswith("///") or stripped.startswith("//!"):
            out.append(stripped)
        elif stripped.startswith("#["):
            pass
        else:
            # A blank line ends a Rust doc comment; do not borrow the doc of
            # the previous item.
            break
        j -= 1
    return "\n".join(reversed(out))


def parse_functions(path: Path, include_tests: bool = False) -> list[Func]:
    relpath = rel(path)
    if not include_tests and (path.name.startswith("tests") or path.name == "tests.rs"):
        return []
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    regions = test_regions(lines)
    impls: list[tuple[int, int, str]] = []
    for i, line in enumerate(lines):
        m = IMPL_RE.match(line)
        if m:
            impls.append((i, brace_block(lines, i), m.group("type")))
    funcs: list[Func] = []
    for i, line in enumerate(lines):
        if COMMENT_ONLY.match(line):
            continue
        if not include_tests and inside(regions, i):
            continue
        m = FN_RE.match(line)
        if not m:
            continue
        name = m.group("name")
        end = brace_block(lines, i)
        impl_type = next((t for a, b, t in impls if a <= i <= b), None)
        body = "\n".join(lines[i : end + 1])
        doc = doc_above(lines, i)
        func = Func(
            path=path,
            rel=relpath,
            name=name,
            line=i + 1,
            doc=doc,
            body=body,
            impl_type=impl_type,
            is_pub=bool(m.group("pub")),
            slice=slice_of(path),
        )
        func.addresses |= implementation_addresses(doc)
        func.addresses |= {a.lower() for a in FUN_IDENT_RE.findall(name)} if "fun_" in name.lower() else set()
        funcs.append(func)
    return funcs


def module_docs(path: Path) -> str:
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    out = [ln.strip() for ln in lines if ln.strip().startswith("//!")]
    return "\n".join(out)


def walk_metadata(obj, key: str = ""):
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield from walk_metadata(v, k.lower())
    elif isinstance(obj, list):
        for v in obj:
            yield from walk_metadata(v, key)
    elif isinstance(obj, str):
        yield key, obj
    elif isinstance(obj, int) and key in {"address", "function"}:
        yield key, f"0x{obj:x}"


def parse_golden(path: Path) -> Golden:
    g = Golden(rel=rel(path), slice=slice_of(path))
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001 - report the file instead of dying
        g.meta.append(f"<invalid JSON: {exc}>")
        return g
    for key, value in walk_metadata(doc):
        if key in {"function", "address", "source", "note", "origin", "module", "live", "tool"}:
            g.meta.append(f"{key}: {value[:200]}")
            g.addresses |= extract_field_addresses(value, key)
            if key == "source":
                g.source = value
            if key == "live":
                g.live = value
    joined = " ".join(g.meta).lower()
    # "a synthetic actor in live calls" is live evidence; only a synthetic
    # *golden* cannot prove the port.
    if (
        "synthetic golden" in joined
        or "crafted golden" in joined
        or "hand-crafted" in joined
        or "fabricated" in joined
        or "not recorded" in joined
        or "does not come from the original" in joined
    ):
        g.synthetic = True
    if g.live == "unknown":
        if any(w in joined for w in ("live", "hidden 1.2", "driver", "snapshot", "unicorn", "executed", "captured")):
            g.live = "yes"
        elif g.synthetic:
            g.live = "synthetic"
        else:
            g.live = "unknown"
    return g


TOL_RE = re.compile(r"\.(?:abs|length|length_squared)\(\)\s*(?:<=?|<)\s*([^;\n]+)")
DELTA_RE = re.compile(r"\b(?:delta|eps|err)\s*(?:<=?|<)\s*([^;\n]+)")
LET_RE = re.compile(r"\blet\s+(?:mut\s+)?([A-Za-z_]\w*)\s*=\s*([^;]+);")


def normalize_code(code: str) -> str:
    return re.sub(r"\s+", " ", code)


def local_values(code: str) -> dict[str, str]:
    return {m.group(1): m.group(2).strip() for m in LET_RE.finditer(normalize_code(code))}


def resolve_tolerance(expr: str, values: dict[str, str], depth: int = 0) -> str:
    expr = re.split(r",\s*[\"']", expr)[0].strip()
    while expr and expr[-1] in ";,":
        expr = expr[:-1].rstrip()
    # Drop only the unmatched closing parens of the surrounding assert!/if.
    while expr.endswith(")") and expr.count(")") > expr.count("("):
        expr = expr[:-1].rstrip()
    if depth < 4 and re.fullmatch(r"[A-Za-z_]\w*", expr) and expr in values:
        return resolve_tolerance(values[expr], values, depth + 1)
    return expr


def tolerances_in(code: str) -> list[str]:
    """Comparisons of the form `(a - b).abs() <= tolerance` in a test/helper."""
    code = normalize_code(code)
    values = local_values(code)
    found = [resolve_tolerance(m.group(1), values) for m in TOL_RE.finditer(code)]
    found += ["delta " + resolve_tolerance(m.group(1), values) for m in DELTA_RE.finditer(code)]
    return [f for f in found if f]


def helper_kind(name: str, body: str) -> tuple[str, list[str]]:
    code = strip_comments(body)
    tol = tolerances_in(code)
    if any(w in name.lower() for w in ("close", "tol", "near", "approx")):
        return "TOLERANCE", tol or ["<tolerance helper>"]
    if tol:
        return "TOLERANCE", tol
    if (
        "assert_eq!" in code
        or "assert_ne!" in code
        or ".to_bits()" in code
        or "as_slice()" in code
        or any(w in name.lower() for w in ("bits", "bytes", "exact", "hex"))
    ):
        return "EXACT", []
    return "UNKNOWN", []


def classify_test(body: str, helpers: dict[str, tuple[str, list[str]]]) -> tuple[str, list[str], bool]:
    code = strip_comments(body)
    kinds: set[str] = set()
    tol: list[str] = []
    exact = False
    for m in re.finditer(r"\b([A-Za-z_]\w*)\s*\(", code):
        h = m.group(1)
        if h in helpers:
            kind, vals = helpers[h]
            if kind == "TOLERANCE":
                kinds.add("TOLERANCE")
                tol.extend(vals)
            elif kind == "EXACT":
                kinds.add("EXACT")
                exact = True
    tol.extend(tolerances_in(code))
    if tol:
        kinds.add("TOLERANCE")
    if "assert_eq!" in code or "assert_ne!" in code:
        kinds.add("EXACT")
        exact = True
    if "matches!(" in code or re.search(r"assert!\([^;]*(?:==|!=)", code):
        kinds.add("EXACT")
        exact = True
    if re.search(r"assert!\([^;]*\)", code):
        kinds.add("EXACT")
        exact = True
    if ".to_bits()" in code or "as_slice()" in code:
        kinds.add("EXACT")
        exact = True
    if "TOLERANCE" in kinds:
        return "TOLERANCE", sorted(set(tol)), exact
    if "EXACT" in kinds:
        return "EXACT", [], True
    return "UNKNOWN", [], False


GENERIC_NAMES = {
    "new", "default", "from", "into", "update", "get", "set", "len", "is_empty",
    "push", "pop", "insert", "remove", "contains", "find", "map", "filter", "iter",
    "collect", "clone", "copy", "to_bytes", "from_bytes", "as_slice", "to_bits",
    "as_f64", "as_u64", "as_i64", "as_str", "as_array", "unwrap", "expect", "format",
    "to_string", "parse", "read", "write", "open", "close", "borrow", "borrow_mut",
    "get_or_insert", "with_capacity", "extend", "zip", "enumerate", "step_by", "abs",
    "max", "min", "clamp", "sqrt", "sin", "cos", "sin_cos", "powi", "floor", "ceil",
    "round", "trunc", "signum", "mul_add", "to_radians", "to_degrees", "f32", "f64",
    "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "usize", "vec", "some",
    "none", "ok", "err", "str", "string", "value", "name", "process", "velocity",
    "position", "rotation", "state", "item", "kind", "entry", "count", "bytes", "time",
    "target", "view", "dir", "phase", "unit", "error", "flags", "point", "scale", "row",
    "rows", "asset", "clip", "id", "x", "y", "z", "v", "b", "d", "m", "t", "a", "n",
}
CALL_RE = re.compile(r"(?:\b(?:super|self|crate)::|\b)([A-Za-z_]\w*)\s*\(")


def test_candidates(test: TestInfo, name_to_funcs: dict[str, list[Func]], helper_names: set[str]) -> list[Func]:
    """Functions a test plausibly exercises: real calls, not every identifier mention."""
    called: dict[str, Func] = {}
    body = strip_comments(test.body)
    for m in CALL_RE.finditer(body):
        name = m.group(1)
        if name in helper_names or name in GENERIC_NAMES:
            continue
        if len(name) < 3:
            continue
        for f in name_to_funcs.get(name, []):
            if f.slice == slice_of(test.path):
                called[f.id] = f
    for m in re.finditer(r"::([A-Za-z_]\w*)\b", body):
        name = m.group(1)
        if name in helper_names or name in GENERIC_NAMES or len(name) < 3:
            continue
        for f in name_to_funcs.get(name, []):
            if f.slice == slice_of(test.path):
                called[f.id] = f
    return list(called.values())


def parse_tests() -> tuple[list[TestInfo], dict[str, list[TestInfo]], list[Func]]:
    tests: list[TestInfo] = []
    all_helpers: list[Func] = []
    test_paths = [
        p
        for p in sorted(PORT.rglob("*.rs"))
        if "goldens" not in p.parts
        and "#[test]" in p.read_text(encoding="utf-8", errors="replace")
    ]
    # `tests_math.rs` and friends are child modules of `tests.rs`; a loader or
    # a match arm defined in the parent must be visible from the child.
    dir_arm: dict[Path, dict[str, Path]] = defaultdict(dict)
    dir_helpers: dict[Path, dict[str, tuple[str, list[str]]]] = defaultdict(dict)
    dir_helper_goldens: dict[Path, dict[str, set[Path]]] = defaultdict(lambda: defaultdict(set))
    dir_funcs: dict[Path, list[Func]] = defaultdict(list)
    for path in test_paths:
        text = path.read_text(encoding="utf-8", errors="replace")
        arm = {name: (path.parent / inc).resolve() for name, inc in LOAD_ARM_RE.findall(text)}
        dir_arm[path.parent].update(arm)
        funcs = parse_functions(path, include_tests=True)
        dir_funcs[path.parent].extend(funcs)
        for f in funcs:
            dir_helpers[path.parent].setdefault(f.name, helper_kind(f.name, f.body))
            incs = INCLUDE_RE.findall(f.body)
            if incs:
                dir_helper_goldens[path.parent][f.name] |= {
                    (path.parent / inc).resolve() for inc in incs
                }
            for name in LOAD_CALL_RE.findall(f.body):
                if name in arm:
                    dir_helper_goldens[path.parent][f.name].add(arm[name])
        # Propagate helper exactness one level (assert_vec -> assert_bits).
        for _ in range(3):
            for f in dir_funcs[path.parent]:
                code = strip_comments(f.body)
                helpers = dir_helpers[path.parent]
                for name, (kind, vals) in list(helpers.items()):
                    if re.search(rf"\b{re.escape(name)}\s*\(", code):
                        if kind == "TOLERANCE" and helpers[f.name][0] != "EXACT":
                            helpers[f.name] = ("TOLERANCE", sorted(set(helpers[f.name][1] + vals)))
                        elif kind == "EXACT" and helpers[f.name][0] == "UNKNOWN":
                            helpers[f.name] = ("EXACT", [])
    for path in test_paths:
        text = path.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        arm_map = dir_arm[path.parent]
        file_funcs = parse_functions(path, include_tests=True)
        all_helpers.extend(f for f in file_funcs if f.name not in {"new"})
        helpers = dir_helpers[path.parent]
        helper_goldens = dir_helper_goldens[path.parent]
        for i, line in enumerate(lines):
            if not TEST_RE.match(line):
                continue
            j = i + 1
            while j < len(lines) and not FN_RE.match(lines[j]):
                j += 1
            if j >= len(lines):
                continue
            m = FN_RE.match(lines[j])
            end = brace_block(lines, j)
            body = "\n".join(lines[j : end + 1])
            ti = TestInfo(path=path, rel=rel(path), name=m.group("name"), line=j + 1, body=body)
            for inc in INCLUDE_RE.findall(body):
                p = (path.parent / inc).resolve()
                if p.exists():
                    ti.goldens.add(rel(p))
            for name in LOAD_CALL_RE.findall(body):
                if name in arm_map:
                    ti.goldens.add(rel(arm_map[name]))
            for h, paths in helper_goldens.items():
                if re.search(rf"\b{re.escape(h)}\s*\(\s*\)", strip_comments(body)):
                    ti.goldens |= {rel(p) for p in paths if p.exists()}
            kind, tol, exact = classify_test(body, helpers)
            ti.kind, ti.tolerance, ti.exact = kind, tol, exact
            if kind == "TOLERANCE" and exact:
                ti.notes.append("test contains both bit/byte-exact and tolerance comparisons")
            tests.append(ti)
    by_golden: dict[str, list[TestInfo]] = defaultdict(list)
    for t in tests:
        for g in t.goldens:
            by_golden[g].append(t)
    return tests, by_golden, all_helpers


def reason_for(func: Func, docs: dict[str, str]) -> tuple[str, str]:
    text = "\n".join((func.doc, func.body, docs.get(func.rel, "")))
    low = text.lower()
    for marker in APPROX_MARKERS:
        if marker in low:
            line = next(
                (ln.strip() for ln in text.splitlines() if marker in ln.lower()),
                marker,
            )
            return "APPROX", line[:240]
    for marker in DECOMPILE_MARKERS:
        if marker in low:
            line = next(
                (ln.strip() for ln in text.splitlines() if marker in ln.lower()),
                marker,
            )
            return "DECOMPILE", line[:240]
    if func.addresses:
        return "DECOMPILE", "ported from the decompile; no live golden found"
    return "REFERENCED", "address named in a comment but no Rust function implements it"


def used_slice_files() -> dict[str, set[str]]:
    """Port slices referenced by ingame/hud code, plus code with comments stripped."""
    used: dict[str, set[str]] = defaultdict(set)
    for root in (INGAME, HUD):
        for path in sorted(root.rglob("*.rs")):
            text = path.read_text(encoding="utf-8", errors="replace")
            code = strip_comments(text)
            for m in re.finditer(r"(?:crate::)?port::([A-Za-z_]\w*)", code):
                if m.group(1) in SLICE_INFO:
                    used[m.group(1)].add(rel(path))
    return used


def wiring(funcs: list[Func]) -> dict[str, str]:
    """Return func.id -> direct | transitive | type-only | no.

    direct      the function name is called by ingame/hud code
    transitive  called by a direct seed through the port call graph
    type-only   only a type from the same file is used; the function itself
                is never called, so it is *not* wired
    no          unreachable from ingame/hud
    """
    used = used_slice_files()
    ingame_code: dict[str, str] = {}
    for root in (INGAME, HUD):
        for path in sorted(root.rglob("*.rs")):
            ingame_code[rel(path)] = strip_comments(path.read_text(encoding="utf-8", errors="replace"))
    types_by_file: dict[str, set[str]] = defaultdict(set)
    for func in funcs:
        path = func.path
        if rel(path) in {f.rel for f in funcs}:
            for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                m = TYPE_RE.match(line)
                if m:
                    types_by_file[func.rel].add(m.group("type"))
    status: dict[str, str] = {}
    for func in funcs:
        direct = False
        for file_rel in used.get(func.slice, set()):
            code = ingame_code.get(file_rel, "")
            # `use ...::{tick as controller_tick}` makes the call site use the
            # alias, not the original name.
            names = {func.name}
            for m in re.finditer(
                rf"\b{re.escape(func.name)}\s+as\s+([A-Za-z_]\w*)", code
            ):
                names.add(m.group(1))
            free = None
            qualified = None
            for name in names:
                esc_name = re.escape(name)
                # A free call, an associated call (`Type::name`) or a path call
                # (`port::module::name`). `x.name(...)` is only trusted when the
                # type that defines it is used in the same file: `map.insert()`
                # must not make InsertState::insert look wired.
                for m in re.finditer(rf"(?<![.\w]){esc_name}\s*\(", code):
                    # `fn add(...)` in ingame/hud is a definition, not a call.
                    if re.search(r"\bfn\s+$", code[max(0, m.start() - 8) : m.start()]):
                        continue
                    free = m
                    break
                qualified = qualified or re.search(rf"::{esc_name}\b", code)
                if free:
                    break
            method = False
            if func.impl_type:
                ty = re.escape(func.impl_type)
                receivers = set()
                for m in re.finditer(
                    rf"\blet\s+(?:mut\s+)?(\w+)\s*(?::\s*[^=;]*\b{ty}\b)?\s*=\s*[^;]*\b{ty}\s*::",
                    code,
                ):
                    receivers.add(m.group(1))
                for m in re.finditer(rf"\b(\w+)\s*:\s*&?\s*(?:mut\s+)?{ty}\b", code):
                    receivers.add(m.group(1))
                for recv in receivers:
                    if re.search(rf"\b{re.escape(recv)}\s*\.\s*{name}\s*\(", code):
                        method = True
                        break
            if free or qualified or method:
                direct = True
                break
        if direct:
            status[func.id] = "direct"
            continue
        typed = False
        for file_rel in used.get(func.slice, set()):
            code = ingame_code.get(file_rel, "")
            if any(re.search(rf"\b{re.escape(t)}\b", code) for t in types_by_file.get(func.rel, set())):
                typed = True
                break
        if typed and func.is_pub:
            status[func.id] = "type-only"
        else:
            status[func.id] = "no"
    # Call graph on free calls only: `foo(...)` or `module::foo(...)`, not
    # `x.foo(...)`. A method call usually means a different function, and the
    # common math method names otherwise produce false "wired" edges.
    by_name: dict[str, list[Func]] = defaultdict(list)
    for f in funcs:
        by_name[f.name].append(f)
    edges: dict[str, set[str]] = defaultdict(set)
    free_call_re = re.compile(r"(?<![.\w])([A-Za-z_]\w*)\s*\(")
    method_call_re = re.compile(r"\.\s*([A-Za-z_]\w*)\s*\(")
    for caller in funcs:
        code = strip_comments(caller.body)
        for m in free_call_re.finditer(code):
            token = m.group(1)
            if token in GENERIC_NAMES or len(token) < 3:
                continue
            callees = by_name.get(token, [])
            if len(callees) == 1 or all(c.rel == caller.rel for c in callees):
                for callee in callees:
                    if callee.id != caller.id:
                        edges[caller.id].add(callee.id)
        # A port method (`self.follow`, `self.update_living_entity`) is a real
        # edge when the name is unique in the port; a method name shared by
        # many types or a standard math method is not.
        for m in method_call_re.finditer(code):
            token = m.group(1)
            if token in GENERIC_NAMES or len(token) < 3:
                continue
            callees = by_name.get(token, [])
            if len(callees) == 1 and callees[0].id != caller.id:
                edges[caller.id].add(callees[0].id)
    seeds = {fid for fid, s in status.items() if s == "direct"}
    queue = list(seeds)
    seen = set(seeds)
    while queue:
        cur = queue.pop()
        for nxt in edges.get(cur, set()):
            if nxt not in seen:
                seen.add(nxt)
                status[nxt] = "transitive"
                queue.append(nxt)
    return status


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out-dir", default="docs/research", help="where the CSV and markdown tables go")
    args = ap.parse_args()
    out_dir = (REPO / args.out_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    port_exists = PORT.exists()
    physics_exists = PHYSICS.exists()
    if not port_exists:
        print(f"error: {PORT} does not exist", file=sys.stderr)
        return 1

    funcs: list[Func] = []
    for path in sorted(PORT.rglob("*.rs")):
        if "goldens" in path.parts:
            continue
        funcs.extend(parse_functions(path))
    if physics_exists:
        for path in sorted(PHYSICS.rglob("*.rs")):
            funcs.extend(parse_functions(path))

    docs = {f.rel: module_docs(f.path) for f in funcs}
    docs = {k: v for k, v in docs.items()}

    # Every address named in the sources, so reference-only mentions are visible.
    occurrences: dict[str, list[tuple[str, int, str]]] = defaultdict(list)
    for path in sorted(PORT.rglob("*.rs")):
        if "goldens" in path.parts:
            continue
        for i, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            comment = line_comment(line)
            if comment:
                for addr in extract_comment_addresses(comment):
                    occurrences[addr].append((rel(path), i, line.strip()[:200]))
            for addr in {a.lower() for a in FUN_IDENT_RE.findall(line)}:
                occurrences[addr].append((rel(path), i, line.strip()[:200]))

    goldens: dict[str, Golden] = {}
    for path in sorted(PORT.rglob("goldens/**/*.json")):
        g = parse_golden(path)
        goldens[g.rel] = g
    tests, by_golden, _ = parse_tests()

    # Which functions a test calls (tokens, not a full Rust parse).
    name_to_funcs: dict[str, list[Func]] = defaultdict(list)
    for f in funcs:
        name_to_funcs[f.name].append(f)
    test_funcs: dict[tuple[str, str], list[Func]] = {}
    test_helper_names: set[str] = set()
    for path in PORT.rglob("*.rs"):
        if path.name.startswith("tests"):
            test_helper_names |= {f.name for f in parse_functions(path, include_tests=True)}
    for t in tests:
        test_funcs[(t.rel, t.name)] = test_candidates(t, name_to_funcs, test_helper_names)

    fn_goldens: dict[str, set[str]] = defaultdict(set)
    review_reasons: dict[str, list[str]] = defaultdict(list)
    # Strong link first: a function's own doc/body names the golden file.
    for f in funcs:
        text = f.doc + "\n" + f.body
        for gpath, g in goldens.items():
            name = Path(gpath).name
            if re.search(rf"(?<![\w]){re.escape(name)}(?![\w])", text):
                fn_goldens[f.id].add(gpath)
    for t in tests:
        called = test_funcs.get((t.rel, t.name), [])
        for gpath in t.goldens:
            g = goldens.get(gpath)
            if g is None:
                review_reasons[gpath].append("test loads a golden that is not under port/goldens")
                continue
            if g.addresses:
                continue  # metadata names its functions; trust it
            linked = {f.id for f in called if gpath in fn_goldens.get(f.id, set())}
            candidates = [f for f in called if f.id not in linked]
            if not linked and len(candidates) > 2:
                review_reasons[gpath].append(
                    f"test {t.name} loads it and calls {len(candidates)} possible port functions: "
                    + ", ".join(sorted({f.name for f in candidates})[:8])
                )
            for f in (candidates if not linked else []):
                fn_goldens[f.id].add(gpath)

    addr_funcs: dict[str, list[Func]] = defaultdict(list)
    for f in funcs:
        for addr in f.addresses:
            addr_funcs[addr].append(f)

    # Recover an address that is only named in a module doc: the golden's test
    # calls exactly one not-yet-addressed function, so that function is the port.
    for gpath, g in goldens.items():
        for addr in list(g.addresses):
            if addr in addr_funcs:
                continue
            candidates = []
            for t in by_golden.get(gpath, []):
                for f in test_funcs.get((t.rel, t.name), []):
                    if not f.addresses:
                        candidates.append(f)
            uniq = {f.id: f for f in candidates}
            if len(uniq) == 1:
                f = next(iter(uniq.values()))
                f.addresses.add(addr)
                addr_funcs[addr].append(f)

    wiring_status = wiring(funcs)

    all_addresses = set(occurrences) | set(addr_funcs) | {
        a for g in goldens.values() for a in g.addresses
    }
    rows: list[dict[str, str]] = []
    for addr in sorted(all_addresses):
        impls = addr_funcs.get(addr, [])
        direct_goldens = sorted(g.rel for g in goldens.values() if addr in g.addresses)
        via_goldens: set[str] = set(direct_goldens)
        for f in impls:
            via_goldens |= fn_goldens.get(f.id, set())
        golden_list = sorted(via_goldens)
        test_kinds: set[str] = set()
        tolerances: set[str] = set()
        test_names: set[str] = set()
        golden_notes: list[str] = []
        untested_goldens: list[str] = []
        for gpath in golden_list:
            g = goldens.get(gpath)
            if g is None:
                continue
            gtests = by_golden.get(gpath, [])
            if g.synthetic:
                golden_notes.append(f"{Path(gpath).name}: source says synthetic/crafted")
            if not gtests:
                untested_goldens.append(Path(gpath).name)
            for t in gtests:
                test_names.add(t.name)
                if t.kind == "TOLERANCE":
                    test_kinds.add("TOLERANCE")
                    tolerances |= set(t.tolerance)
                elif t.kind == "EXACT":
                    test_kinds.add("EXACT")
                else:
                    test_kinds.add("UNKNOWN")
                golden_notes.extend(t.notes)
        # An untested golden is evidence of nothing; only report it when no
        # linked golden has a test at all.
        if untested_goldens and not test_names:
            test_kinds.add("UNTESTED")
            golden_notes.append(
                "live capture but no test loads " + ", ".join(untested_goldens)
            )
        elif untested_goldens:
            golden_notes.append(
                "also unloaded: " + ", ".join(untested_goldens)
            )
        if not impls:
            base_level = "GOLDEN-ONLY" if golden_list and addr not in occurrences else "REFERENCED"
        elif "TOLERANCE" in test_kinds:
            base_level = "TOLERANCE"
        elif "EXACT" in test_kinds:
            base_level = "EXACT"
        elif impls:
            base_level, why = reason_for(impls[0], docs)
            golden_notes.append(why)
        else:
            base_level = "REFERENCED"
        # A concrete approximation marker outranks the absence of a live case.
        if impls and base_level == "DECOMPILE":
            why_level, why = reason_for(impls[0], docs)
            if why_level == "APPROX":
                base_level = "APPROX"
                golden_notes.append(why)
        if addr in LEVEL_OVERRIDES:
            base_level, why = LEVEL_OVERRIDES[addr]
            golden_notes.append(why)
        statuses = {wiring_status.get(f.id, "no") for f in impls}
        if not impls:
            wiring_label = "none"
        elif "direct" in statuses:
            wiring_label = "direct"
        elif "transitive" in statuses:
            wiring_label = "transitive"
        elif "type-only" in statuses:
            wiring_label = "type-only"
        else:
            wiring_label = "no"
        wired = wiring_label in {"direct", "transitive"}
        level = base_level
        if base_level in {"EXACT", "TOLERANCE", "DECOMPILE", "APPROX"} and not wired:
            level = base_level + "+UNWIRED"
        if not impls:
            location = occurrences.get(addr, [("", 0, "")])[0]
            fileline = f"{location[0]}:{location[1]}" if location[0] else ""
            fn_names = ""
        else:
            first = impls[0]
            fileline = f"{first.rel}:{first.line}"
            fn_names = "; ".join(sorted({f"{f.name} ({f.rel}:{f.line})" for f in impls}))
        manual = set()
        if not impls and golden_list and addr not in occurrences:
            manual.add("golden-only address: no ported source names it")
        if impls and "UNTESTED" in test_kinds:
            manual.add("live capture exists but no test loads it (kept DECOMPILE)")
        if "UNKNOWN" in test_kinds:
            manual.add("test comparison style could not be classified")
        if impls and not impls[0].doc and base_level == "DECOMPILE":
            manual.add("address is only on a module/type doc, not the function doc")
        rows.append(
            {
                "slice": impls[0].slice if impls else slice_of(Path(golden_list[0])) if golden_list else slice_of(Path(occurrences.get(addr, [("", 0, "")])[0][0])),
                "address": addr,
                "level": level,
                "base_level": base_level,
                "wired": wiring_label,
                "rust_fn": fn_names,
                "file_line": fileline,
                "goldens": "; ".join(Path(g).name for g in golden_list),
                "golden_paths": "; ".join(golden_list),
                "tests": "; ".join(sorted(test_names)),
                "comparison": (
                    (
                        "tolerance: " + "; ".join(sorted(tolerances))
                        if "TOLERANCE" in test_kinds
                        else "bit/byte exact"
                        if "EXACT" in test_kinds
                        else ""
                    )
                    if impls
                    else ""
                ),
                "reason": " | ".join(sorted(set(golden_notes)))[:600],
                "manual_review": "yes" if manual else "no",
                "review_reason": "; ".join(sorted(manual)),
            }
        )

    # Golden-only addresses have no source mention; they are still listed so a
    # golden cannot silently prove a function that is not in the port.
    known = {r["address"] for r in rows}
    for g in sorted(goldens.values(), key=lambda x: x.rel):
        for addr in sorted(g.addresses - known):
            rows.append(
                {
                    "slice": g.slice,
                    "address": addr,
                    "level": "GOLDEN-ONLY",
                    "base_level": "GOLDEN-ONLY",
                    "wired": "none",
                    "rust_fn": "",
                    "file_line": g.rel,
                    "goldens": Path(g.rel).name,
                    "golden_paths": g.rel,
                    "tests": "; ".join(sorted({t.name for t in by_golden.get(g.rel, [])})),
                    "comparison": "",
                    "reason": "golden names this address but no ported source does",
                    "manual_review": "yes",
                    "review_reason": "golden-only address",
                }
            )

    slice_rank = {s: i for i, s in enumerate(SLICE_ORDER)}
    rows.sort(key=lambda r: (slice_rank.get(r["slice"], 99), LEVEL_ORDER.get(r["base_level"], 99), r["address"]))

    csv_path = out_dir / "proof-levels.csv"
    fields = [
        "slice",
        "address",
        "level",
        "base_level",
        "wired",
        "rust_fn",
        "file_line",
        "goldens",
        "golden_paths",
        "tests",
        "comparison",
        "reason",
        "manual_review",
        "review_reason",
    ]
    with csv_path.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)

    table_path = out_dir / "proof-levels-table.md"
    with table_path.open("w", encoding="utf-8") as fh:
        fh.write("# Proof levels (generated)\n\n")
        fh.write("Generated by `tools/port_audit/proof_levels.py`; do not edit by hand. ")
        fh.write("`wired` is direct/type/transitive/no (see the script docstring).\n\n")
        fh.write("| slice | address | level | wired | Rust function | source | goldens | comparison |\n")
        fh.write("|---|---|---|---|---|---|---|---|\n")
        for r in rows:
            fh.write(
                "| {slice} | `{address}` | {level} | {wired} | {rust_fn} | {file_line} | {goldens} | {comparison} |\n".format(
                    **{k: (r[k] or "").replace("|", "\\|") for k in r}
                )
            )

    review_path = out_dir / "proof-levels-review.md"
    review = [r for r in rows if r["manual_review"] == "yes"]
    reference_only = [r for r in rows if r["base_level"] == "REFERENCED"]
    with review_path.open("w", encoding="utf-8") as fh:
        fh.write("# Proof-level rows for manual review (generated)\n\n")
        fh.write(f"{len(review)} of {len(rows)} rows need a decision. ")
        fh.write("Review each against the test source; fix the script's heuristic rather than the row.\n\n")
        fh.write(
            f"The other {len(reference_only)} non-EXACT rows are **reference-only**: addresses of "
            "original functions that a port comment names but that this tree does not implement. "
            "They are classified `REFERENCED`, not unproven port code; the CSV/table still lists "
            "each one with its file and line.\n\n"
        )
        by_reason: dict[str, list[dict[str, str]]] = defaultdict(list)
        for r in review:
            for reason in r["review_reason"].split("; "):
                by_reason[reason].append(r)
        for reason in sorted(by_reason):
            fh.write(f"## {reason} ({len(by_reason[reason])})\n\n")
            fh.write("| slice | address | level | Rust function | source | goldens |\n")
            fh.write("|---|---|---|---|---|---|\n")
            for r in sorted(by_reason[reason], key=lambda x: (x["slice"], x["address"])):
                fh.write(
                    "| {slice} | `{address}` | {level} | {rust_fn} | {file_line} | {goldens} |\n".format(
                        **{k: (r[k] or "").replace("|", "\\|") for k in r}
                    )
                )
            fh.write("\n")

    # Console summary.
    base_counts = Counter(r["base_level"] for r in rows)
    wire_counts = Counter(r["wired"] for r in rows)
    print(f"aa-physics: {'present' if physics_exists else 'MISSING (crates/aa-physics not in this worktree)'}")
    print(f"rows: {len(rows)}  manual review: {len(review)}")
    print("levels:", dict(sorted(base_counts.items())))
    print("wiring:", dict(sorted(wire_counts.items())))
    print()
    print("slice,EXACT,TOLERANCE,DECOMPILE,APPROX,REFERENCED,GOLDEN-ONLY,UNWIRED,total")
    for s in SLICE_ORDER:
        srows = [r for r in rows if r["slice"] == s]
        if not srows and s not in {"", "physics"}:
            continue
        c = Counter(r["base_level"] for r in srows)
        unwired = sum(1 for r in srows if r["level"].endswith("+UNWIRED"))
        print(f"{s or '(unknown)'},{c['EXACT']},{c['TOLERANCE']},{c['DECOMPILE']},{c['APPROX']},{c['REFERENCED']},{c['GOLDEN-ONLY']},{unwired},{len(srows)}")
    print()
    for path in (csv_path, table_path, review_path):
        try:
            shown = path.relative_to(REPO).as_posix()
        except ValueError:
            shown = os.path.relpath(path, REPO)
        print(f"wrote {shown}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
