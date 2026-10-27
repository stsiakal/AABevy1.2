"""Index the Ghidra x2game decompile by function start address.

Writes data/decompile_validation/x2game_funcs.tsv with:
  addr, start_line, end_line, params, float10, dat_refs, decl
"""

import argparse
import os
import re
import sys

HEADER_RE = re.compile(r"^// ==== (\S+) @ ([0-9a-fA-F]+)\s*$")


def split_args(s):
    parts, depth, cur = [], 0, ""
    for ch in s:
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
            continue
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        cur += ch
    if cur.strip():
        parts.append(cur)
    return parts


def parse(path):
    funcs = []
    cur = None
    lines = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for lineno, raw in enumerate(f, 1):
            line = raw.rstrip("\n")
            m = HEADER_RE.match(line)
            if m:
                if cur is not None:
                    cur["end_line"] = lineno - 1
                    cur["text"] = "\n".join(lines)
                    funcs.append(cur)
                cur = {"name": m.group(1), "addr": int(m.group(2), 16),
                       "start_line": lineno, "decl": "", "params": None}
                lines = []
                continue
            if cur is None:
                continue
            lines.append(line)
            if cur["params"] is None:
                s = line.strip()
                if not s or s.startswith("/*") or s.startswith("//") or s.startswith("*"):
                    continue
                if "(" in s:
                    # accumulate until the parameter list closes
                    if cur["decl"]:
                        cur["decl"] += " " + s
                    else:
                        cur["decl"] = s
                    if ")" in cur["decl"].split("(", 1)[1]:
                        d = cur["decl"]
                        args = d.split("(", 1)[1].rsplit(")", 1)[0].strip()
                        cur["params"] = 0 if args in ("", "void") else len(split_args(args))
    if cur is not None:
        cur["end_line"] = lineno
        cur["text"] = "\n".join(lines)
        funcs.append(cur)
    return funcs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default=".")
    ap.add_argument("--decompile", default="D:/aa-re/client12/out-x2game/x2game.dll.c")
    ap.add_argument("--out", default="data/decompile_validation/x2game_funcs.tsv")
    args = ap.parse_args()
    root = os.path.abspath(args.root)
    funcs = parse(args.decompile)
    out = os.path.join(root, args.out)
    dat_re = re.compile(r"\bDAT_([0-9a-fA-F]{8})\b")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("addr\tstart_line\tend_line\tparams\tfloat10\tdat_refs\tdecl\n")
        for fn in sorted(funcs, key=lambda x: x["addr"]):
            text = fn["text"]
            fl = text.count("float10")
            dats = len(set(dat_re.findall(text)))
            f.write("%08x\t%d\t%d\t%s\t%d\t%d\t%s\n" % (
                fn["addr"], fn["start_line"], fn["end_line"],
                "" if fn["params"] is None else fn["params"],
                fl, dats, fn["decl"].replace("\t", " ")[:180]))
    print("functions=%d -> %s" % (len(funcs), out))


if __name__ == "__main__":
    sys.exit(main())
