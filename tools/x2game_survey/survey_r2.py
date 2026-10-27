#!/usr/bin/env python
"""Read-only survey helper for the x2game.dll function ranges.

Only stdlib; run with ``python -I tools/x2game_survey/survey.py <cmd>``.

Commands:
  summary                 function/byte totals for the survey range
  anchors [--lo HEX] [--hi HEX] [--step HEX]
                          address-ordered functions that carry structural strings
  dump --lo HEX --hi HEX  every function in an address window with its strings
  func ADDR               one function: strings, callers, callees
  coverage                mark functions cited in docs/research/port-*/MAP.md and
                          crates/*/src as MAPPED/PORTED candidates
  areas FILE.json         aggregate the hand-written area table and verify totals
"""
import argparse
import csv
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = "D:/aa-re/client12/out-x2game"
FUNCS = os.path.join(OUT, "funcs.csv")
XREFS = os.path.join(OUT, "strings-xrefs.tsv")
CALLS = os.path.join(OUT, "calls.tsv")

LO = 0x3926DE70
HI = 0x394BEC40

_STRUCT = re.compile(
    r"(::|^X2[A-Za-z]|^C[A-Z][A-Za-z0-9_]{3,}$|^[A-Z][A-Za-z0-9]+(Info|Packet|Request|Response|Notify|Msg)$"
    r"|\.(xml|dds|cgf|chr|caf|dba|pak|txt|alb|lua|mtl|skin|anm|wav|ogg|bsp|gfx|ttf)$"
    r"|^(CMF|CBK|CURRENCY|SND|UI|ITEM|QUEST|SKILL|CHAT|HOUSE|FARM|VEHICLE|MATE|SLAVE)_"
    r"|^[A-Z][A-Za-z0-9]+_[A-Z][A-Za-z0-9_]+$)"
    r"|X2Log|ScriptBind|Lua|Widget|Manager|^ui/|^game/|^sound/",
    re.I,
)
_NOISE = re.compile(r"^(%[\w.*\- ]*|[0-9.+\-*/=<> ]+|true|false|null|nil|none|OK|Failed|Error)$", re.I)


def norm_addr(s):
    s = s.strip()
    if s.startswith("0x") or s.startswith("0X"):
        s = s[2:]
    try:
        return int(s, 16)
    except ValueError:
        return None


def load_funcs(lo=LO, hi=HI, all_funcs=False):
    rows = []
    with open(FUNCS, newline="") as f:
        for r in csv.DictReader(f):
            a = norm_addr(r["start"])
            if a is None:
                continue
            if all_funcs or lo <= a < hi:
                rows.append({"start": a, "end": norm_addr(r["end"]),
                             "size": int(r["size"]), "name": r["name"]})
    rows.sort(key=lambda r: r["start"])
    return rows


def load_strings():
    by_fn = {}
    with open(XREFS, newline="", encoding="utf-8", errors="replace") as f:
        for r in csv.DictReader(f, delimiter="\t"):
            a = norm_addr(r["function_addr"])
            if a is None:
                continue
            by_fn.setdefault(a, []).append(r["string"])
    return by_fn


def load_calls():
    calls = {}
    rcall = {}
    with open(CALLS, newline="") as f:
        for r in csv.DictReader(f, delimiter="\t"):
            a = norm_addr(r["caller_addr"])
            b = norm_addr(r["callee_addr"])
            if a is None or b is None:
                continue
            calls.setdefault(a, set()).add(b)
            rcall.setdefault(b, set()).add(a)
    return calls, rcall


def fmt_addr(a):
    return "0x%x" % a


def structural(strings):
    out = []
    seen = set()
    for s in strings:
        s = s.strip()
        if s in seen or len(s) < 4 or len(s) > 140:
            continue
        if _NOISE.match(s):
            continue
        if _STRUCT.search(s):
            seen.add(s)
            out.append(s)
    return out


def cmd_summary(args):
    rows = load_funcs(args.lo, args.hi)
    total = sum(r["size"] for r in rows)
    print("range %s..%s  functions=%d  bytes=%d  KB=%.1f"
          % (fmt_addr(args.lo), fmt_addr(args.hi), len(rows), total, total / 1024))


def cmd_anchors(args):
    rows = load_funcs(args.lo, args.hi)
    strs = load_strings()
    for r in rows:
        ss = structural(strs.get(r["start"], []))
        if not ss:
            continue
        print("%s\t%d\t%s\t%s" % (fmt_addr(r["start"]), r["size"], r["name"], " | ".join(ss[:14])))


def cmd_dump(args):
    rows = load_funcs(args.lo, args.hi)
    strs = load_strings()
    for r in rows:
        ss = strs.get(r["start"], [])
        print("%s\t%d\t%s\t%s" % (fmt_addr(r["start"]), r["size"], r["name"], " | ".join(ss)))
    total = sum(r["size"] for r in rows)
    print("# %d fns, %.1f KB" % (len(rows), total / 1024), file=sys.stderr)


def cmd_func(args):
    a = norm_addr(args.addr)
    rows = {r["start"]: r for r in load_funcs(0, 1 << 32, all_funcs=True)}
    r = rows.get(a)
    if not r:
        print("no function at %s" % fmt_addr(a))
        return
    strs = load_strings().get(a, [])
    calls, rcall = load_calls()
    print("%s %s size=%d end=%s" % (r["name"], fmt_addr(a), r["size"], fmt_addr(r["end"])))
    print("strings: " + " | ".join(strs))
    print("callees: " + " ".join(fmt_addr(x) for x in sorted(calls.get(a, []))))
    print("callers: " + " ".join(fmt_addr(x) for x in sorted(rcall.get(a, []))))


def _hexes(text):
    out = set()
    for m in re.finditer(r"0x(39[0-9a-fA-F]{6})", text):
        out.add(int(m.group(1), 16))
    return out


def cmd_coverage(args):
    rows = load_funcs(args.lo, args.hi)
    docs = set()
    paths = []
    base = os.path.join(ROOT, "docs", "research")
    for d in os.listdir(base):
        p = os.path.join(base, d, "MAP.md")
        if os.path.isfile(p):
            paths.append(p)
    for p in paths:
        with open(p, encoding="utf-8", errors="replace") as f:
            docs |= _hexes(f.read())
    src = set()
    for sub in ("crates/aa-client/src/port", "crates/aa-ui", "crates/aa-net",
                "crates/aa-world", "crates/aa-cry", "crates/aa-fx", "crates/aa-audio"):
        root = os.path.join(ROOT, sub)
        for dp, _dn, fn in os.walk(root):
            for f in fn:
                if not f.endswith((".rs", ".md")):
                    continue
                try:
                    with open(os.path.join(dp, f), encoding="utf-8", errors="replace") as fh:
                        src |= _hexes(fh.read())
                except OSError:
                    pass
    if args.lo == LO and args.hi == HI:
        pass
    hit_doc = sum(r["size"] for r in rows if r["start"] in docs)
    hit_src = sum(r["size"] for r in rows if r["start"] in src)
    print("range funcs=%d KB=%.1f; cited in MAP.md: %d fns %.1f KB; cited in crates: %d fns %.1f KB"
          % (len(rows), sum(r["size"] for r in rows) / 1024,
             sum(1 for r in rows if r["start"] in docs), hit_doc / 1024,
             sum(1 for r in rows if r["start"] in src), hit_src / 1024))
    if args.list:
        for r in rows:
            if r["start"] in docs or r["start"] in src:
                where = ("doc" if r["start"] in docs else "") + ("+src" if r["start"] in src else "")
                print("%s\t%d\t%s\t%s" % (fmt_addr(r["start"]), r["size"], where, r["name"]))


def cmd_decomp(args):
    a = norm_addr(args.addr)
    idx = {}
    with open(os.path.join(OUT, "_index.txt"), encoding="utf-8", errors="replace") as f:
        for line in f:
            parts = line.split("\t")
            if len(parts) >= 2:
                try:
                    idx[int(parts[0], 16)] = int(parts[1])
                except ValueError:
                    pass
    start = idx.get(a)
    if start is None:
        print("no decompile slice at %s" % fmt_addr(a))
        return
    with open(os.path.join(OUT, "x2game.dll.c"), "rb") as f:
        f.seek(start)
        text = f.read(args.bytes).decode("utf-8", "replace")
    for line in text.splitlines()[:args.lines]:
        print(line)


def cmd_areas(args):
    with open(args.file, encoding="utf-8") as f:
        spec = json.load(f)
    rows = load_funcs(args.lo, args.hi)
    covered = set()
    total_f = total_b = 0
    status_f = {}
    status_b = {}
    print("| area (what it does) | start | fns | KB | runtime | status | evidence |")
    print("|---|---|---:|---:|---|---|---|")
    for area in spec["areas"]:
        lo, hi = norm_addr(area["lo"]), norm_addr(area["hi"])
        sub = [r for r in rows if lo <= r["start"] < hi and r["start"] not in covered]
        f, b = len(sub), sum(r["size"] for r in sub)
        covered |= {r["start"] for r in sub}
        total_f += f
        total_b += b
        st = area["status"]
        status_f[st] = status_f.get(st, 0) + f
        status_b[st] = status_b.get(st, 0) + b
        print("| %s | %s | %d | %.1f | %s | %s | %s |"
              % (area["area"], fmt_addr(lo), f, b / 1024,
                 area.get("runtime", "no" if st == "OUT" else "yes"),
                 st, area.get("evidence", "")))
    rest = [r for r in rows if r["start"] not in covered]
    print("# covered %d/%d fns, %.1f/%.1f KB; unassigned %d fns %.1f KB"
          % (total_f, len(rows), total_b / 1024, sum(r["size"] for r in rows) / 1024,
             len(rest), sum(r["size"] for r in rest) / 1024))
    for st in sorted(status_f):
        print("# status %-8s %5d fns %8.1f KB" % (st, status_f[st], status_b[st] / 1024))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--lo", type=lambda s: norm_addr(s if s.startswith("0x") else s), default=LO)
    ap.add_argument("--hi", type=lambda s: norm_addr(s if s.startswith("0x") else s), default=HI)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("summary")
    p = sub.add_parser("anchors")
    p.add_argument("--step", type=lambda s: int(s, 0), default=0)
    p = sub.add_parser("dump")
    p.add_argument("--list", action="store_true")
    p = sub.add_parser("func")
    p.add_argument("addr")
    p = sub.add_parser("coverage")
    p.add_argument("--list", action="store_true")
    p = sub.add_parser("decomp")
    p.add_argument("addr")
    p.add_argument("--lines", type=int, default=60)
    p.add_argument("--bytes", type=int, default=40000)
    p = sub.add_parser("areas")
    p.add_argument("file")
    args = ap.parse_args()
    {"summary": cmd_summary, "anchors": cmd_anchors, "dump": cmd_dump,
     "func": cmd_func, "coverage": cmd_coverage, "areas": cmd_areas,
     "decomp": cmd_decomp}[args.cmd](args)


if __name__ == "__main__":
    main()
