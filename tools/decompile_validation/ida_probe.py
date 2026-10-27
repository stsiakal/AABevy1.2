"""Read-only IDA probe: prologue/epilogue and ret imm16 for cited addresses.

Env:
  AA_TARGETS  file with one hex address per line
  AA_OUT      output text file
"""

import os

import ida_auto
import ida_funcs
import ida_ua

ida_auto.auto_wait()

out_path = os.environ["AA_OUT"]
targets = []
with open(os.environ["AA_TARGETS"], encoding="utf-8") as f:
    for line in f:
        line = line.strip()
        if line and not line.startswith("#"):
            targets.append(int(line, 16))


def text(ea):
    insn = ida_ua.insn_t()
    if not ida_ua.decode_insn(insn, ea):
        return "?"
    s = insn.get_canon_mnem()
    if insn.ops[0].type != 0:
        s += " %x" % insn.ops[0].value
    return s


with open(out_path, "w", encoding="utf-8") as f:
    for ea in targets:
        fn = ida_funcs.get_func(ea)
        f.write("== %08x\n" % ea)
        if fn is None:
            f.write("no function\n\n")
            continue
        f.write("start=%08x end=%08x size=%d\n" % (fn.start_ea, fn.end_ea, fn.end_ea - fn.start_ea))
        f.write("prologue:")
        cur = fn.start_ea
        for _ in range(8):
            f.write(" [%s]" % text(cur))
            insn = ida_ua.insn_t()
            if not ida_ua.decode_insn(insn, cur):
                break
            cur += insn.size
        f.write("\nepilogue:")
        # walk back up to 40 bytes and print decoded instructions
        cur = max(fn.start_ea, fn.end_ea - 40)
        for _ in range(20):
            if cur >= fn.end_ea:
                break
            f.write(" [%s]" % text(cur))
            insn = ida_ua.insn_t()
            if not ida_ua.decode_insn(insn, cur):
                break
            cur += insn.size
        f.write("\n")
        # direct ret search
        cur = fn.start_ea
        rets = []
        while cur < fn.end_ea:
            insn = ida_ua.insn_t()
            if not ida_ua.decode_insn(insn, cur):
                cur += 1
                continue
            if insn.get_canon_mnem() in ("ret", "retn"):
                rets.append((cur, insn.ops[0].value if insn.ops[0].type not in (0,) else 0))
                if len(rets) >= 6:
                    break
            cur += insn.size
        f.write("rets: %s\n\n" % ", ".join("%x:%d" % r for r in rets))
