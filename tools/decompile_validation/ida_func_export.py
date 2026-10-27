"""Read-only IDA export for decompile validation.

Writes <AA_OUT>/<AA_NAME>.funcs.csv with start/end/size/name plus the
disassembly facts the audit needs on x86 (no Hex-Rays there):

  args_bytes : stack bytes from `ret imm16` (-1 when no ret is found)
  first_ops  : up to six canonical mnemonics of the function prologue
  type       : IDA's one-line function type, if the database has one

Run headless:
  AA_OUT=... AA_NAME=... idat.exe -A -B "-S<this file>" <dll>
"""

import os

import ida_auto
import ida_bytes
import ida_funcs
import ida_typeinf
import ida_ua
import idautils
import idc

ida_auto.auto_wait()

out = os.environ["AA_OUT"]
name = os.environ["AA_NAME"]
os.makedirs(out, exist_ok=True)
path = os.path.join(out, name)


def first_mnemonics(ea, end, limit=6):
    insn = ida_ua.insn_t()
    ops = []
    cur = ea
    while cur < end and len(ops) < limit:
        if not ida_ua.decode_insn(insn, cur):
            break
        ops.append(insn.get_canon_mnem())
        cur += insn.size
    return " ".join(ops)


def ret_imm(ea, end):
    insn = ida_ua.insn_t()
    cur = ea
    while cur < end:
        if not ida_ua.decode_insn(insn, cur):
            cur += 1
            continue
        if insn.get_canon_mnem() in ("ret", "retn", "retf"):
            if insn.ops[0].type != 0 and insn.ops[0].type != ida_ua.o_void:
                return int(insn.ops[0].value)
            return 0
        cur += insn.size
    return -1


def type_line(ea):
    try:
        t = idc.get_type(ea)
        return t.replace("\t", " ").replace(",", ";") if t else ""
    except Exception:
        return ""


count = 0
with open(path + ".funcs.csv", "w", encoding="utf-8") as f:
    f.write("start,end,size,name,args_bytes,first_ops,type\n")
    for ea in idautils.Functions():
        fn = ida_funcs.get_func(ea)
        if fn is None:
            continue
        count += 1
        f.write(
            "0x%X,0x%X,%d,%s,%d,%s,%s\n"
            % (
                ea,
                fn.end_ea,
                fn.end_ea - ea,
                ida_funcs.get_func_name(ea),
                ret_imm(ea, fn.end_ea),
                first_mnemonics(ea, fn.end_ea),
                type_line(ea),
            )
        )

with open(path + ".export.log", "w", encoding="utf-8") as f:
    f.write("funcs=%d\n" % count)
