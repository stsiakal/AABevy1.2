#!/usr/bin/env python3
"""Record formula-engine goldens from the running 1.2 client.

Talks to the in-client driver (`aaemu-clientdriver`, native `aadriver.dll`) over its named pipe
and uses the `call`/`read`/`alloc`/`write`/`free` operations.  Nothing is written outside the
driver's tracked scratch allocations; the parser nodes are heap objects created by the original
and only read back.

Usage: python tools/capture_formulas.py --pid <client pid> [--out DIR]
"""

import argparse
import json
import math
import os
import re
import struct
import sys

VARS = [float(i) * 0.5 + 1.0 for i in range(35)]
BASE_RVA = 0x38FF0000

TOKEN_END = 1
TOKEN_IDENT = 3
TOKEN_NUMBER = 4
TOKEN_PUNCT = {5: "+", 6: "-", 7: "*", 8: "/", 9: "^", 10: "(", 11: ")", 12: ","}


def f32_bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def bits_f32(bits):
    return struct.unpack("<f", struct.pack("<I", bits & 0xFFFFFFFF))[0]


def result_json(bits):
    value = bits_f32(bits)
    if math.isnan(value):
        return {"bits": None, "nan": True}
    if math.isinf(value):
        return {"bits": f32_bits(value), "nan": False}
    return {"bits": bits, "nan": False}


class Driver:
    def __init__(self, pid):
        self.pipe = open(r"\\.\pipe\aaemu-clientdriver-%d" % pid, "r+b", buffering=0)
        text = self.request("M", "x2game.dll")
        match = re.search(r"base=0x([0-9a-fA-F]+)", text)
        if not match:
            raise RuntimeError("module reply: " + text)
        self.base = int(match.group(1), 16)

    def request(self, op, payload):
        data = op.encode() + payload.encode()
        self.pipe.write(struct.pack("<I", len(data)) + data)
        header = self._read_exact(4)
        length = struct.unpack("<I", header)[0]
        reply = self._read_exact(length)
        status, body = reply[:1], reply[1:].decode("utf-8", "replace")
        if status == b"0":
            return body
        raise RuntimeError(body)

    def _read_exact(self, count):
        out = b""
        while len(out) < count:
            chunk = self.pipe.read(count - len(out))
            if not chunk:
                raise RuntimeError("pipe closed")
            out += chunk
        return out

    def alloc(self, size):
        text = self.request("A", str(size))
        match = re.search(r"0x([0-9a-fA-F]+)", text)
        if not match:
            raise RuntimeError("alloc reply: " + text)
        return int(match.group(1), 16)

    def free(self, address):
        self.request("D", "0x%08x" % address)

    def write(self, address, data):
        self.request("w", "%s %s" % (address, data.hex()))

    def write_words(self, address, words):
        self.write(address, struct.pack("<%dI" % len(words), *words))

    def write_f32s(self, address, values):
        self.write(address, struct.pack("<%df" % len(values), *values))

    def read(self, address, length):
        text = self.request("R", "%s %d" % (address, length))
        match = re.search(r"(?:^|\s)hex=([0-9a-fA-F]*)", text)
        if not match:
            raise RuntimeError("read reply: " + text)
        return bytes.fromhex(match.group(1))

    def read_words(self, address, count):
        return list(struct.unpack("<%dI" % count, self.read(address, count * 4)))

    def call(self, target, conv, args=(), this=None, dumps=(), unsafe=False):
        parts = [target, conv]
        if this is not None:
            parts.append("this=0x%08x" % this)
        for address, length in dumps:
            parts.append("dump=0x%08x:%d" % (address, length))
        if unsafe:
            parts.append("unsafe")
        parts.extend(args)
        text = self.request("c", " ".join(parts))
        fields = {}
        for token in text.split():
            if "=" in token:
                key, value = token.split("=", 1)
                fields[key] = value
        if "eax" not in fields or "d64" not in fields:
            raise RuntimeError("call reply: " + text)
        eax = int(fields["eax"], 16)
        value = struct.unpack("<d", bytes.fromhex(fields["d64"]))[0]
        try:
            value = struct.unpack("<f", struct.pack("<f", value))[0]
        except OverflowError:
            value = math.copysign(float("inf"), value)
        buffers = []
        index = 0
        while "buf%d" % index in fields:
            buffers.append(bytes.fromhex(fields["buf%d" % index]))
            index += 1
        return eax, value, buffers


def parse_float(text):
    if text == "NaN":
        return float("nan")
    if text == "inf":
        return float("inf")
    if text == "-inf":
        return float("-inf")
    return float(text)


def float_bits_from_text(text):
    value = parse_float(text)
    if math.isnan(value):
        return None
    return f32_bits(value)


def f32_arg(bits):
    return "u32:%d" % (bits & 0xFFFFFFFF)


def ptr(address):
    return "p:0x%08x" % address


def u32(value):
    return "u32:%d" % (value & 0xFFFFFFFF)


def module_rva(base, va):
    return "x2game.dll+0x%x" % (va - BASE_RVA)


class Capture:
    def __init__(self, driver, out_dir):
        self.d = driver
        self.out = out_dir
        self.scratch = driver.alloc(0x2000)
        self.token = driver.alloc(0x200)
        self.text = driver.alloc(0x400)
        self.arena = driver.alloc(0x1000)
        table_rva = 0x15CD4C0
        addresses = driver.read_words(driver.base + table_rva, 24)
        self.evaluators = addresses
        os.makedirs(out_dir, exist_ok=True)

    def save(self, name, data):
        path = os.path.join(self.out, name)
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(data, handle, indent=1, sort_keys=True)
            handle.write("\n")
        print("wrote", path)

    def write_text(self, text):
        raw = text.encode("utf-8") + b"\0"
        self.d.write(self.text, raw)
        return self.text

    def zero_context(self):
        self.d.write(self.scratch, b"\0" * 0x400)

    def parse(self, text):
        address = self.write_text(text)
        self.zero_context()
        eax, _, _ = self.d.call(module_rva(self.d.base, 0x398B0B20), "cdecl", [ptr(self.scratch + 0xC), ptr(address)])
        if eax == 0:
            return None
        return eax

    def read_tree(self, address, depth=0):
        if depth > 12:
            raise RuntimeError("tree too deep at 0x%08x" % address)
        words = self.d.read_words(address, 5)
        node_type = words[1]
        if node_type == 0:
            return {"t": 0, "v": words[2]}
        if node_type == 1:
            return {"t": 1, "i": words[2] & 0xFFFFFFFF}
        if node_type in (2, 3, 4, 5, 6, 16, 17):
            return {
                "t": node_type,
                "a": self.read_tree(words[2], depth + 1),
                "b": self.read_tree(words[3], depth + 1),
            }
        if node_type in (7, 8, 9, 10, 11, 18, 19, 20, 21):
            return {"t": node_type, "a": self.read_tree(words[2], depth + 1)}
        if node_type in (12, 13, 14, 15):
            return {
                "t": node_type,
                "a": self.read_tree(words[2], depth + 1),
                "b": self.read_tree(words[3], depth + 1),
                "c": self.read_tree(words[4], depth + 1),
            }
        if node_type == 22:
            argc = words[3]
            arg_addresses = self.d.read_words(words[4], argc)
            return {
                "t": 22,
                "id": words[2] & 0xFFFFFFFF,
                "args": [self.read_tree(arg, depth + 1) for arg in arg_addresses],
            }
        raise RuntimeError("unexpected node type %d" % node_type)

    def evaluate_tree(self, address):
        words = self.d.read_words(address, 5)
        node_type = words[1]
        target = "0x%08x" % self.evaluators[node_type]
        _, value, _ = self.d.call(target, "cdecl", [ptr(address)], unsafe=True)
        if math.isnan(value):
            return {"bits": None, "nan": True}
        return {"bits": f32_bits(value), "nan": False}

    # ---------------------------------------------------------------- lexer states

    def capture_lexer_states(self):
        cases = []
        functions = [
            ("FUN_398afbf0", 0x398AFBF0, 0),
            ("FUN_398afde0", 0x398AFDE0, 1),
            ("FUN_398afef0", 0x398AFEF0, 2),
            ("FUN_398b0030", 0x398B0030, 3),
        ]
        bytes_ = [0, ord("("), ord(")"), ord("*"), ord("+"), ord(","), ord("-"), ord("."),
                  ord("/"), ord("^"), ord("a"), ord("_"), ord("5"), ord(" "), 9,
                  ord("!"), 0x80, ord("A"), ord("z"), ord("9")]
        for name, va, state in functions:
            for byte in bytes_:
                self.d.write(self.scratch, b"\0" * 0x20)
                self.d.write(self.token, b"\0" * 0x110)
                words = [state, 0, 0, 0, 0, self.token]
                self.d.write_words(self.scratch, words)
                eax, _, buffers = self.d.call(
                    module_rva(self.d.base, va), "thiscall", [u32(byte)],
                    this=self.scratch, dumps=[(self.scratch, 0x20), (self.token, 0x20)])
                state_words = struct.unpack("<8I", buffers[0])
                token_words = struct.unpack("<8I", buffers[1])
                cases.append({
                    "function": name,
                    "state": state,
                    "byte": byte,
                    "advance": (eax & 0xFF) != 0,
                    "new_state": state_words[0],
                    "token": token_words[0],
                    "len": state_words[4],
                    "buffer": buffers[1][4:4 + state_words[4]].hex(),
                })
        self.save("lexer_states.json", {"cases": cases})

    def capture_lexer_driver(self):
        cases = []
        texts = ["", " ", "abc", "abc123", "a_b", "1", "1.5", ".5", "1.", "1..2", "1e2",
                 "a.b", "123abc", "+", "-", "*", "/", "^", "(", ")", ",", "foo bar", "!!",
                 "a+b", " 42 ", "0.0", "level+1", "min(1,2)"]
        for text in texts:
            address = self.write_text(text)
            self.d.write(self.scratch, b"\0" * 0x20)
            self.d.write(self.token, b"\0" * 0x110)
            words = [0, 0, address, 0, 0, self.token]
            self.d.write_words(self.scratch, words)
            _, _, buffers = self.d.call(
                module_rva(self.d.base, 0x398B0450), "thiscall", [ptr(self.token)],
                this=self.scratch, dumps=[(self.scratch, 0x20), (self.token, 0x120)])
            state_words = struct.unpack("<8I", buffers[0])
            token_type = struct.unpack_from("<I", buffers[1], 0)[0]
            pos = state_words[2] - address
            number_bits = None
            if token_type == TOKEN_NUMBER:
                number_bits = struct.unpack_from("<I", buffers[1], 4)[0]
            buffer = b""
            if token_type in (TOKEN_IDENT, TOKEN_NUMBER):
                buffer = buffers[1][4:0x104].split(b"\0", 1)[0]
            cases.append({
                "text": text,
                "pos": pos,
                "token": token_type,
                "number_bits": number_bits,
                "identifier": buffer.decode("latin-1") if token_type == TOKEN_IDENT else None,
                "error": state_words[6] & 0xFF,
            })
        self.save("lexer_driver.json", {"cases": cases})

    # ---------------------------------------------------------------- lookups

    def capture_lookups(self):
        names = ["level_diff", "casting_tolerance", "damage_percent", "source_level",
                 "target_level", "stealth_level", "npc_level", "pc_level", "range",
                 "experience", "str", "dex", "sta", "int", "spi", "fai", "level", "ab_level",
                 "level_dps", "facets", "npc_size_avg", "labor_power", "char_race",
                 "npc_template", "npc_kind", "mate_kind", "npc_grade", "item_level",
                 "item_grade", "armor", "magic_resist", "equip_slot_enchant_cost",
                 "battle_resist", "bulls_eye", "flexibility", "FVK_MAX", "bogus", "LEVEL", ""]
        variables = []
        for name in names:
            address = self.write_text(name)
            eax, _, _ = self.d.call(module_rva(self.d.base, 0x39568140), "cdecl", [ptr(address)])
            variables.append({"name": name, "index": eax})
        functions = []
        for name in ["none", "FFK_MAX", "min", "max", "clamp", "if_zero", "if_positive",
                     "if_negative", "floor", "log", "sqrt", "abs", "foo", "", "NONE", "None",
                     "none(1)", "lvl", "level_diff", "flexibility"]:
            address = self.write_text(name)
            eax, _, _ = self.d.call(module_rva(self.d.base, 0x39568180), "cdecl", [ptr(address)])
            functions.append({"name": name, "index": eax})
        self.save("lookups.json", {"variables": variables, "functions": functions})

    # ---------------------------------------------------------------- parser tree cases

    def capture_parser(self):
        groups = {
            "FUN_398b0b20": [
                "0", "-1", "1.5", "1+2*3", "2^3^2", "-2^2", "2^-3^2",
                "min(3, 5) + level * 2", "max(1,2)", "clamp(-1,0,10)", "if_zero(0,7,8)",
                "if_positive(1,7,8)", "if_negative(-1,7,8)", "floor(2.9)", "log(100)",
                "sqrt(2)", "abs(-3)", "str + level", "pi", "1/0", "1 2 +", "((1)", "a.b",
                "min(1)", "clamp(1,2)", "foo(1)", "", "1e2", "1.2.3", "-",
            ],
            "FUN_398b0a70": [
                "1+2", "1-2", "1+2+3", "1-2+3", "1+2-3", "1-2-3", "level+str", "level-str",
                "1+2*3", "1*2+3", "1-2*3", "1*2-3", "min(1,2)+max(3,4)", "(1+2)+3",
                "1+(2+3)", "1+-2", "1--2", "1++2", "-1+2", "2*-3+4",
            ],
            "FUN_398b09c0": [
                "1*2", "1/2", "1*2*3", "1/2/3", "1*2/3", "1/2*3", "level*str", "level/str",
                "1+2*3", "1*2+3", "1-2*3", "1*2-3", "min(1,2)*2", "2*min(1,2)", "(1+2)*3",
                "2*(1+3)", "1*-2", "1/-2", "-1*2", "level*2/4",
            ],
            "FUN_398b08a0": [
                "2^3", "2^3^2", "-2^2", "2^-3", "2^-3^2", "2^+3", "2^3*4", "2*3^2",
                "(2^3)^2", "2^(3^2)", "level^2", "sqrt(2)^2", "min(2,3)^2", "2^min(2,3)",
                "2^-2", "-2^-2", "2^3^1", "1^0", "0^0", "2^0.5",
            ],
            "FUN_398b0840": [
                "-1", "+1", "--1", "-+-1", "-level", "+level", "-min(1,2)", "-2^2", "2^-3",
                "1+-2", "1--2", "1*-2", "-(-2)", "+(-2)", "-str", "-1*2", "-1+2", "-1-2",
                "-(1+2)", "+++5",
            ],
            "FUN_398b0680": [
                "0", "1", "1.5", ".5", "1.", "level", "str", "(1)", "((1))", "(1+2)",
                "(level)", "pi", "(1", "1)", "()", "a.b", "1e2", "min(1,2)", "none(1)", "-1",
            ],
            "FUN_398b0560": [
                "min(1,2)", "max(1,2)", "clamp(1,2,3)", "if_zero(0,1,2)",
                "if_positive(1,1,2)", "if_negative(-1,1,2)", "floor(1.5)", "log(10)",
                "sqrt(4)", "abs(-2)", "min(1+2,3+4)", "max(min(1,2),3)",
                "clamp(level,0,100)", "if_zero(level,str,dex)", "if_positive(level,str,dex)",
                "if_negative(level,str,dex)", "floor(level*1.5)", "log(level)", "sqrt(level)",
                "abs(level-str)",
            ],
            "FUN_398b0150": [
                "min(1,2)", "max(1,2)", "clamp(1,2,3)", "if_zero(0,1,2)",
                "if_positive(1,1,2)", "if_negative(-1,1,2)", "floor(1.5)", "log(10)",
                "sqrt(4)", "abs(-2)", "min(1)", "max(1,2,3)", "clamp(1,2)", "if_zero(1)",
                "if_positive(1,2,3,4)", "if_negative()", "floor(1,2)", "log()",
                "sqrt(1,2)", "abs()",
            ],
        }
        cases = []
        for function, expressions in groups.items():
            for expression in expressions:
                node = self.parse(expression)
                case = {"function": function, "text": expression, "ok": node is not None}
                if node is not None:
                    case["tree"] = self.read_tree(node)
                    self.d.write_f32s(self.scratch + 0x1C, VARS)
                    case["result"] = self.evaluate_tree(node)
                cases.append(case)
        extra = [
            "foo(1)", "FFK_MAX(1)", "none(1)", "min()", "clamp(1,2,3,4)",
            "1/0", "0/0", "1/(-0)", "sqrt(-1)", "log(0)", "log(-1)",
            "if_zero(-0,1,2)", "if_positive(-0,1,2)", "if_negative(-0,1,2)",
        ]
        for expression in extra:
            node = self.parse(expression)
            case = {"function": "FUN_398b0150", "text": expression, "ok": node is not None}
            if node is not None:
                case["tree"] = self.read_tree(node)
                self.d.write_f32s(self.scratch + 0x1C, VARS)
                case["result"] = self.evaluate_tree(node)
            cases.append(case)
        self.save("parser.json", {"vars": [f32_bits(v) for v in VARS], "cases": cases})

    # ---------------------------------------------------------------- evaluator leaves

    def build_arena(self, spec):
        nodes = []
        strings = []

        def layout(item):
            start = len(nodes) * 0x14 if nodes else 0
            nodes.append(None)
            index = len(nodes) - 1
            if item["t"] == 22:
                args = [layout(arg) for arg in item["args"]]
                args_offset = 0x800 + len(strings) * 0x40
                strings.append(struct.pack("<%dI" % len(args), *args))
                nodes[index] = {"t": 22, "id": item["id"], "argc": len(args), "args": args_offset}
            elif item["t"] == 23:
                child = layout(item["a"])
                name_offset = 0x800 + len(strings) * 0x40
                strings.append(item["name"].encode("latin-1") + b"\0")
                nodes[index] = {"t": 23, "child": child, "name": name_offset}
            elif item["t"] in (2, 3, 4, 5, 6, 16, 17):
                nodes[index] = {"t": item["t"], "a": layout(item["a"]), "b": layout(item["b"])}
            elif item["t"] in (7, 8, 9, 10, 11, 18, 19, 20, 21):
                nodes[index] = {"t": item["t"], "a": layout(item["a"])}
            elif item["t"] in (12, 13, 14, 15):
                nodes[index] = {"t": item["t"], "a": layout(item["a"]),
                                "b": layout(item["b"]), "c": layout(item["c"])}
            elif item["t"] in (0, 1):
                nodes[index] = dict(item)
            else:
                raise RuntimeError("bad node type")
            return start

        root = layout(spec)
        arena = bytearray(0x1000)
        context = self.scratch + 0xC
        base = self.arena
        for index, node in enumerate(nodes):
            offset = index * 0x14
            struct.pack_into("<I", arena, offset + 0, context)
            struct.pack_into("<I", arena, offset + 4, node["t"])
            if node["t"] == 0:
                struct.pack_into("<I", arena, offset + 8, node["v"])
            elif node["t"] == 1:
                struct.pack_into("<I", arena, offset + 8, node["i"])
            elif node["t"] in (2, 3, 4, 5, 6, 16, 17):
                struct.pack_into("<I", arena, offset + 8, base + node["a"])
                struct.pack_into("<I", arena, offset + 12, base + node["b"])
            elif node["t"] in (7, 8, 9, 10, 11, 18, 19, 20, 21):
                struct.pack_into("<I", arena, offset + 8, base + node["a"])
            elif node["t"] in (12, 13, 14, 15):
                struct.pack_into("<I", arena, offset + 8, base + node["a"])
                struct.pack_into("<I", arena, offset + 12, base + node["b"])
                struct.pack_into("<I", arena, offset + 16, base + node["c"])
            elif node["t"] == 22:
                struct.pack_into("<I", arena, offset + 8, node["id"])
                struct.pack_into("<I", arena, offset + 12, node["argc"])
                struct.pack_into("<I", arena, offset + 16, base + node["args"])
            elif node["t"] == 23:
                struct.pack_into("<I", arena, offset + 8, base + node["name"])
                struct.pack_into("<I", arena, offset + 12, base + node["child"])
        for index, data in enumerate(strings):
            arena[0x800 + index * 0x40:0x800 + index * 0x40 + len(data)] = data
        return arena, root

    def capture_nodes(self):
        def const(value):
            return {"t": 0, "v": f32_bits(value)}

        def var(index):
            return {"t": 1, "i": index}

        values = [0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, -2.0, 3.5, 100.0, -100.0,
                  1e6, 1e-6, 3.4028234663852886e38, 1.1754943508222875e-38, 123.456,
                  -7.25, 10.0, 0.1, 3.0]
        special = [float("nan"), float("inf"), float("-inf")]
        pairs = [(values[i], values[(i * 7 + 3) % len(values)]) for i in range(20)]
        cases = []

        def add(node_type, spec, vars_=None):
            arena, root = self.build_arena(spec)
            self.zero_context()
            self.d.write_f32s(self.scratch + 0x1C, VARS)
            self.d.write(self.arena, bytes(arena))
            target = "0x%08x" % self.evaluators[node_type]
            _, value, _ = self.d.call(target, "cdecl", [ptr(self.arena + root)], unsafe=True)
            if math.isnan(value):
                result = {"bits": None, "nan": True}
            else:
                result = {"bits": f32_bits(value), "nan": False}
            case = {"type": node_type, "label": "%d_%d" % (node_type, len([c for c in cases if c["type"] == node_type])),
                    "tree": spec, "result": result}
            if vars_ is not None:
                case["vars"] = vars_
            cases.append(case)

        for value in values:
            add(0, const(value))
        for index in list(range(18)) + [34, 35]:
            add(1, var(index), [f32_bits(v) for v in VARS])

        for node_type in (2, 3, 4, 5):
            for first, second in pairs:
                add(node_type, {"t": node_type, "a": const(first), "b": const(second)})

        pow_pairs = [(2.0, 3.0), (2.0, 0.5), (-2.0, 3.0), (-2.0, 2.0), (-2.0, 0.5),
                     (0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (10.0, -2.0), (100.0, 0.5),
                     (2.0, 10.0), (2.0, -3.0), (0.5, 2.0), (4.0, 0.5), (3.0, 3.0),
                     (1.5, 2.5), (2.0, 1e6), (1e6, 2.0), (2.0, float("nan")),
                     (-1.0, 0.5)]
        for base, exponent in pow_pairs:
            add(6, {"t": 6, "a": const(base), "b": const(exponent)})

        for value in values:
            add(7, {"t": 7, "a": const(value)})

        trig_values = [0.0, 1.0, -1.0, 0.5, -0.5, 3.141592653589793, -3.141592653589793,
                       1.5707963267948966, -1.5707963267948966, 0.7853981633974483,
                       6.283185307179586, 10.0, -10.0, 100.0, 1e6, 1e-6, 2.0, -2.0,
                       float("nan"), float("inf")]
        for node_type in (8, 9, 10, 11):
            for value in trig_values:
                add(node_type, {"t": node_type, "a": const(value)})

        clamp_values = [(-1.0, 0.0, 10.0), (11.0, 0.0, 10.0), (5.0, 0.0, 10.0),
                        (0.0, 0.0, 10.0), (10.0, 0.0, 10.0), (0.1, 0.1, 10.0),
                        (0.1, 0.0, 0.1), (0.0, 0.0, 0.0), (-0.0, 0.0, 10.0),
                        (5.0, 10.0, 0.0), (5.0, 5.0, 5.0), (2.0, 1.0, 3.0),
                        (3.0, 1.0, 3.0), (4.0, 1.0, 3.0), (1.0, 1.0, 3.0),
                        (float("nan"), 0.0, 1.0), (0.5, float("nan"), 1.0),
                        (0.5, 0.0, float("nan")), (float("inf"), 0.0, 10.0),
                        (float("-inf"), 0.0, 10.0)]
        for value, low, high in clamp_values:
            add(12, {"t": 12, "a": const(value), "b": const(low), "c": const(high)})

        if_values = [(0.0, 7.0, 8.0), (-0.0, 7.0, 8.0), (1.0, 7.0, 8.0), (-1.0, 7.0, 8.0),
                     (1e-6, 7.0, 8.0), (-1e-6, 7.0, 8.0), (100.0, 7.0, 8.0),
                     (-100.0, 7.0, 8.0), (float("nan"), 7.0, 8.0), (float("inf"), 7.0, 8.0),
                     (float("-inf"), 7.0, 8.0), (0.5, -1.0, 1.0), (-0.5, -1.0, 1.0),
                     (2.0, 3.0, 4.0), (-2.0, 3.0, 4.0), (1.0, float("nan"), 8.0),
                     (0.0, float("nan"), 8.0), (-1.0, 7.0, float("nan")),
                     (3.0, 1.0, 2.0), (-3.0, 1.0, 2.0)]
        for node_type in (13, 14, 15):
            for condition, then_value, else_value in if_values:
                add(node_type, {"t": node_type, "a": const(condition),
                                "b": const(then_value), "c": const(else_value)})

        minmax_pairs = [(1.0, 2.0), (2.0, 1.0), (1.0, 1.0), (-1.0, 2.0), (-2.0, -1.0),
                        (0.0, -0.0), (-0.0, 0.0), (100.0, -100.0), (0.1, 0.2),
                        (float("nan"), 5.0), (5.0, float("nan")), (float("inf"), 5.0),
                        (5.0, float("inf")), (float("-inf"), 5.0), (5.0, float("-inf")),
                        (3.5, 2.5), (2.5, 3.5), (1e6, 1e-6), (1e-6, 1e6), (7.0, 0.0)]
        for node_type in (16, 17):
            for first, second in minmax_pairs:
                add(node_type, {"t": node_type, "a": const(first), "b": const(second)})

        for value in values:
            add(18, {"t": 18, "a": const(value)})

        log_values = [1.0, 10.0, 100.0, 0.5, 2.0, 1000.0, 0.1, 3.0, 5.0, 7.0,
                      1e6, 1e-6, 3.4028234663852886e38, 1.1754943508222875e-38,
                      0.0, -0.0, -1.0, float("nan"), float("inf"), 42.0]
        for value in log_values:
            add(19, {"t": 19, "a": const(value)})

        sqrt_values = [0.0, -0.0, 1.0, 2.0, 4.0, 0.25, 0.5, 9.0, 16.0, 100.0,
                       -1.0, -4.0, 1e6, 1e-6, 3.4028234663852886e38,
                       1.1754943508222875e-38, float("nan"), float("inf"), 7.0, 42.0]
        for value in sqrt_values:
            add(20, {"t": 20, "a": const(value)})

        abs_values = [0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 100.0, -100.0, 1e6, -1e6,
                      1e-6, -1e-6, float("nan"), float("inf"), float("-inf"), 3.5,
                      -3.5, 7.25, -7.25, 42.0]
        for value in abs_values:
            add(21, {"t": 21, "a": const(value)})

        for index in range(20):
            argc = index % 4
            args = [const(values[(index + j) % len(values)]) for j in range(argc)]
            node_id = [0, 0, 0, 1, 2, 0xFFFFFFFF][index % 6]
            add(22, {"t": 22, "id": node_id, "args": args})

        # Type 23 (table path) is not produced by the formula parser and its lookup requires a
        # live game-expression descriptor table; synthetic names fault in FUN_395692b0.  It is
        # ported as an unreachable node and has no live goldens.

        self.save("nodes.json", {"cases": cases})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--out", default=os.path.join("crates", "aa-client", "src", "port",
                                                      "formulas", "goldens"))
    args = parser.parse_args()
    driver = Driver(args.pid)
    capture = Capture(driver, args.out)
    capture.capture_lexer_states()
    capture.capture_lexer_driver()
    capture.capture_lookups()
    capture.capture_parser()
    capture.capture_nodes()
    for address in (capture.scratch, capture.token, capture.text, capture.arena):
        driver.free(address)
    print("done")


if __name__ == "__main__":
    sys.exit(main())
