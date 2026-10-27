"""Regenerate Appendix A/B of docs/research/decompile-validation.md."""

import csv
import os


def short(s, n):
    return s if len(s) <= n else s[: n - 1] + "..."


root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
base = os.path.join(root, "data", "decompile_validation")
doc = os.path.join(root, "docs", "research", "decompile-validation.md")

with open(os.path.join(base, "globals.tsv"), encoding="utf-8") as f:
    globals_ = list(csv.DictReader(f, delimiter="\t"))
with open(os.path.join(base, "vtables.tsv"), encoding="utf-8") as f:
    vtables = list(csv.DictReader(f, delimiter="\t"))
with open(os.path.join(base, "float10.tsv"), encoding="utf-8") as f:
    float10 = [r for r in csv.DictReader(f, delimiter="\t")
               if "crates" in r["port_refs"] and r["float10"] != "NO_DECOMPILE"]
with open(os.path.join(base, "test_styles.tsv"), encoding="utf-8") as f:
    styles = {r["module"]: r for r in csv.DictReader(f, delimiter="\t")}

lines = ["", "## Appendix A. Every DAT_ cited by port code (%d)" % len(globals_), "",
         "| DAT | class | section / bytes | first port reference |", "|---|---|---|---|"]
for r in sorted(globals_, key=lambda r: int(r["addr"], 16)):
    value = r["value"] if r["value"] != "-" else "-"
    ref = r["port_refs"].split(";")[0]
    lines.append("| `%s` | %s | `%s` | `%s` |" % (
        r["addr"], r["class"], short(value, 52), short(ref, 72)))

lines += ["", "## Appendix B. Every vtable slot cited by port code (%d)" % len(vtables), "",
          "| slot | class | first port reference |", "|---|---|---|"]
for r in sorted(vtables, key=lambda r: int(r["slot"], 16)):
    ref = r["port_refs"].split(";")[0]
    lines.append("| `%s` | %s | `%s` |" % (r["slot"], r["class"], short(ref, 110)))

lines += ["", "## Appendix C. Every float10 path cited by port code (%d)" % len(float10), "",
          "| function | float10 | module | module test style | first port reference |",
          "|---|---:|---|---|---|"]
for r in sorted(float10, key=lambda r: (-int(r["float10"]), r["addr"])):
    ref = next((x for x in r["port_refs"].split(";") if "crates" in x), "")
    parts = ref.replace("\\", "/").split("/")
    mod = parts[parts.index("port") + 1] if "port" in parts and len(parts) > parts.index("port") + 1 else "?"
    st = styles.get(mod)
    if st:
        exact, approx, tol = int(st["assert_eq"]), int(st["approx"]), int(st["float_tol"])
        if approx == 0 and tol == 0:
            style = "EXACT (%d assert_eq)" % exact
        elif exact == 0:
            style = "TOLERANCE (%d approx/%d tol)" % (approx, tol)
        else:
            style = "MIXED (%d exact/%d approx/%d tol)" % (exact, approx, tol)
    else:
        style = "?"
    lines.append("| `%s` | %s | %s | %s | `%s` |" % (
        r["addr"], r["float10"], mod, style, short(ref, 80)))

text = open(doc, encoding="utf-8").read()
cut = text.find("## Appendix A.")
if cut >= 0:
    text = text[:cut].rstrip() + "\n"
else:
    text = text.rstrip() + "\n"
open(doc, "w", encoding="utf-8", newline="\n").write(text + "\n".join(lines) + "\n")
print("appendix: %d DAT, %d slots" % (len(globals_), len(vtables)))
