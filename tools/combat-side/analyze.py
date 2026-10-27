#!/usr/bin/env python3
"""Print the paired original/Bevy combat facts for docs/research/port-combat/side-by-side.md.

The two captures in a run directory are:
  * original: meta.json, playanim.csv, controller_samples.csv, server_slice.log
  * Bevy:     bevy_meta.json, feel.csv.marks.csv, bevy.log, bevy_server_slice.log

Times are seconds from the original's wall_start_ms and from the Bevy feel START mark.
"""

from __future__ import annotations

import csv
import json
import re
import sys
from pathlib import Path


def load_csv(path: Path, name: str) -> list[dict[str, str]]:
    with path.open(encoding="utf-8", errors="replace", newline="") as f:
        return list(csv.DictReader(f))


def first_change(rows: list[dict[str, str]], key: str, t0: float) -> list[tuple[float, str, str]]:
    out: list[tuple[float, str, str]] = []
    prev: str | None = None
    for row in rows:
        value = row.get(key, "")
        if prev is not None and value != prev:
            out.append(((float(row["wall_ms"]) - t0) / 1000.0, prev, value))
        prev = value
    return out


def first_transition(rows, key, old, new, t0):
    for t, a, b in first_change(rows, key, t0):
        if a == old and b == new:
            return t
    return None


def parse_server(path: Path) -> dict:
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    start: dict[int, tuple[str, str]] = {}
    started: dict[int, str] = {}
    fired: dict[int, str] = {}
    damaged: list[str] = []
    death: str | None = None
    cleared: list[str] = []
    for i, line in enumerate(lines):
        if "CSStartSkillPacket len=13 body=" in line:
            body = line.split("len=13 body=")[1].strip()
            fields = next((x.strip() for x in lines[i:i + 4] if "CSStartSkillPacket fields:" in x), "")
            m = re.search(r"skill_id: (\d+)", fields)
            if m:
                start.setdefault(int(m.group(1)), (body, fields))
        elif "SCSkillStartedPacket {" in line:
            m = re.search(r"id: (\d+)", line)
            if m:
                started.setdefault(int(m.group(1)), line)
        elif "SCSkillFiredPacket {" in line:
            m = re.search(r"id: (\d+)", line)
            if m:
                fired.setdefault(int(m.group(1)), line)
        if "SCUnitDamagedPacket {" in line:
            damaged.append(line.strip())
        if "SCUnitDeathPacket {" in line:
            death = line.strip()
        if "SCCombatClearedPacket {" in line:
            cleared.append(line.strip())
    return {"start": start, "started": started, "fired": fired,
            "damaged": damaged, "death": death, "cleared": cleared}


def parse_bevy(run: Path) -> dict:
    meta = json.loads((run / "bevy_meta.json").read_text(encoding="utf-8"))
    feel = load_csv(run / "feel.csv", "feel")
    meta["player_id"] = int(feel[0]["player_obj"]) if feel else None
    meta["exit_code"] = meta.get("returncode")
    marks = {r["name"]: float(r["ms"]) for r in load_csv(run / "feel.csv.marks.csv", "marks")}
    start_ms = marks.get("START", 0.0) / 1000.0
    asked: dict[int, float] = {}
    asked_targets: list[int] = []
    events: list[dict] = []
    caster_hits: list[dict] = []
    victim_hits: list[dict] = []
    stance: list[dict] = []
    gcds: list[dict] = []
    for line in (run / "bevy.log").read_text(encoding="utf-8", errors="replace").splitlines():
        m = re.search(r"combat: \[([\d.]+)\] asked for skill (\d+) on (\d+)", line)
        if m:
            asked.setdefault(int(m.group(2)), float(m.group(1)))
            asked_targets.append(int(m.group(3)))
            continue
        if "CA f=" in line:
            continue
        m = re.match(r"CA t=([\d.]+) (\w+)(.*)", line)
        if not m:
            continue
        t, kind, rest = float(m.group(1)), m.group(2), m.group(3)
        ev = {"t": t, "kind": kind, "raw": line, "rest": rest}
        events.append(ev)
        if kind == "gcd":
            gm = re.search(r"skill=(\d+) total=(\d+) left=(\d+)", rest)
            if gm:
                gcds.append({"t": t, "skill": int(gm.group(1)),
                             "total": int(gm.group(2)), "left": int(gm.group(3))})
            continue
        if kind == "hit":
            cm = re.search(r"c=(\d+) v=(\d+)", rest)
            am = re.search(r"a=(\d+)", rest)
            if cm and am:
                tm = re.search(r"tl=(\d+)", rest)
                hit = {"t": t, "c": int(cm.group(1)), "v": int(cm.group(2)),
                       "a": int(am.group(1)), "rest": rest,
                       "tl": int(tm.group(1)) if tm else None}
                if hit["c"] == meta.get("player_id"):
                    victim_hits.append(hit)
                elif hit["v"] == meta.get("player_id"):
                    caster_hits.append(hit)
        elif kind == "stance":
            sm = re.search(r"o=(\d+) perm=(\w+) in=(\w+)(?: deadline=(\d+))?", rest)
            if sm:
                stance.append({"t": t, "o": int(sm.group(1)),
                               "perm": sm.group(2), "in": sm.group(3),
                               "deadline": int(sm.group(4)) if sm.group(4) else None})
    first = {}
    for ev in events:
        first.setdefault((ev["kind"], re.search(r"a=(\d+)", ev["rest"]).group(1) if "a=" in ev["rest"] else ""), ev["t"])
    meta["target_id"] = asked_targets[0] if asked_targets else None
    server = parse_server(run / "bevy_server_slice.log")
    return {"meta": meta, "marks": marks, "start": start_ms, "asked": asked,
            "events": events, "caster_hits": caster_hits, "victim_hits": victim_hits,
            "stance": stance, "gcds": gcds, "first": first, "server": server}


def main() -> int:
    run = Path(sys.argv[1] if len(sys.argv) > 1 else "shots/combat-side/final")
    om = json.loads((run / "meta.json").read_text(encoding="utf-8"))
    t0 = om["wall_start_ms"]
    o_rows = load_csv(run / "controller_samples.csv", "controller")
    o_pa = load_csv(run / "playanim.csv", "playanim")
    o_server = parse_server(run / "server_slice.log")
    b = parse_bevy(run)

    def o_pa_first(anim: int, ctrl: int | None = None, after: float = -1.0) -> float | None:
        for r in o_pa:
            if int(r["anim_id"]) != anim:
                continue
            if ctrl is not None and int(r["ctrl"]) != ctrl:
                continue
            t = (float(r["wall_ms"]) - t0) / 1000.0
            if t >= after:
                return t
        return None

    caster = int(om["caster_ctrl"])
    victim = int(om["victim_ctrl"])
    def o_c(t): return round(t, 3) if t is not None else None
    o_weapon_draws = [t for t, a, b in first_change(o_rows, "weapon_active", t0)
                      if a == "0" and b == "1"]

    print("ORIGINAL", {k: om[k] for k in ("character", "player_id", "victim_id", "victim_name", "target_distance")})
    print("BEVY", {k: b["meta"].get(k) for k in ("character", "player_id", "npc", "target_id", "exit_code")})
    print("inputs original", [(i["at"], i["value"]) for i in om["inputs"]])
    print("START/END marks", {k: b["marks"].get(k) for k in ("START", "END")})
    print("asked", b["asked"])

    print("\nORIGINAL 18132: input", om["inputs"][0]["at"],
          "plot", o_c(o_pa_first(296, caster)),
          "297", o_c(o_pa_first(297, caster)),
          "298", o_c(o_pa_first(298, caster)),
          "target14", o_c(o_pa_first(14, victim)),
          "GCD", o_c(first_transition(o_rows, "gcd_total", "0", "2000", t0)),
          "weapon", o_c(first_transition(o_rows, "weapon_active", "0", "1", t0)),
          "stance", o_c(first_transition(o_rows, "stance_perm", "0", "1", t0)))
    print("ORIGINAL 10955: input", om["inputs"][1]["at"],
          "cast_start", o_c(first_transition(o_rows, "cast", "0", "130", t0)),
          "cast_end", o_c(first_transition(o_rows, "cast", "130", "0", t0)),
          "fire", o_c(first_transition(o_rows, "cast", "130", "0", t0)),
          "target14", o_c(o_pa_first(14, victim, 6.0)))
    print("ORIGINAL auto: input", om["inputs"][2]["at"],
          "weapon_draw", o_c(next((t for t in o_weapon_draws if t > 7.0), None)),
          "auto87", o_c(o_pa_first(87, caster, 8.0)),
          "stance_off", o_c(first_transition(o_rows, "stance_perm", "1", "0", t0)))
    print("ORIGINAL stance perm", [(round(t, 3), a, b) for t, a, b in first_change(o_rows, "stance_perm", t0)])
    print("ORIGINAL combat_in", [(round(t, 3), a, b) for t, a, b in first_change(o_rows, "combat_in", t0)])
    print("ORIGINAL gcd totals", [(round(t, 3), a, b) for t, a, b in first_change(o_rows, "gcd_total", t0)])
    reactions = {"14", "15", "17", "417", "418"}
    print("ORIGINAL reactions (anim, data=tl)",
          [(round((float(r["wall_ms"]) - t0) / 1000.0, 3), int(r["ctrl"]), int(r["anim_id"]), int(r["data"]))
           for r in o_pa if r["anim_id"] in reactions])
    print("ORIGINAL CS", {k: v[0] for k, v in o_server["start"].items()})
    print("ORIGINAL SC started", {k: v for k, v in o_server["started"].items()})
    print("ORIGINAL SC fired", {k: v for k, v in o_server["fired"].items()})
    print("ORIGINAL death", o_server["death"])
    print("ORIGINAL cleared", o_server["cleared"])

    def b_ev(kind: str, **want) -> list[dict]:
        out = []
        for e in b["events"]:
            if e["kind"] != kind:
                continue
            if all(re.search(rf"{k}=([^ ]+)", e["rest"]) and int(re.search(rf"{k}=(\d+)", e["rest"]).group(1)) == v
                   for k, v in want.items()):
                out.append(e)
        return out

    def bt(t: float) -> float: return round(t - b["start"], 3)
    print("\nBEVY CA events")
    for e in b["events"]:
        if e["kind"] in {"plot", "cast", "fire", "hit", "stance", "swap", "start", "end"}:
            print(f"  {bt(e['t']):7.3f} {e['kind']:6s} {e['rest'].strip()}")
    def tl(h: dict) -> int | None:
        m = re.search(r"tl=(\d+)", h.get("rest", ""))
        return int(m.group(1)) if m else None

    print("BEVY target hits", [(bt(h["t"]), tl(h), h["a"]) for h in b["victim_hits"]])
    print("BEVY caster hits", [(bt(h["t"]), tl(h), h["a"]) for h in b["caster_hits"]])
    print("BEVY stance", [(bt(s["t"]), s["o"], s["perm"], s["in"], s.get("deadline")) for s in b["stance"]])
    print("BEVY gcd", [(bt(g["t"]), g["skill"], g["total"], g["left"]) for g in b["gcds"]])
    print("BEVY CS", {k: v[0] for k, v in b["server"]["start"].items()})
    print("BEVY started", {k: v for k, v in b["server"]["started"].items()})
    print("BEVY fired", {k: v for k, v in b["server"]["fired"].items()})
    print("BEVY death", b["server"]["death"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
