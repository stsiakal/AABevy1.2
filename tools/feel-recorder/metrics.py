"""Numbers the report asks for: stride period, rider bob, rider-vs-mount yaw."""

import math

import numpy as np


def _sub(rows, t0, t1):
    return [r for r in rows if t0 <= r["t_ms"] / 1000.0 <= t1]


def _series(rows, key):
    t, v = [], []
    for r in rows:
        x = r.get(key)
        if isinstance(x, (int, float)):
            t.append(r["t_ms"] / 1000.0)
            v.append(float(x))
    return np.array(t), np.array(v)


def _resample(rows, key, t0, t1, hz=100.0):
    t, v = _series(rows, key)
    if len(t) < 8:
        return None, None
    mask = (t >= t0) & (t <= t1)
    t, v = t[mask], v[mask]
    if len(t) < 8:
        return None, None
    order = np.argsort(t)
    t, v = t[order], v[order]
    grid = np.arange(max(t0, t[0]), min(t1, t[-1]), 1.0 / hz)
    if len(grid) < 16:
        return None, None
    return grid, np.interp(grid, t, v)


def _seat(rows):
    """Rider pelvis height above the mount root (z cancels terrain and path differences)."""
    for r in rows:
        rz, mz = r.get("rider_z"), r.get("mount_z")
        r["seat_z"] = (float(rz) - float(mz)) if isinstance(rz, (int, float)) and isinstance(mz, (int, float)) else None


def _stride(rows, t0=1.0, t1=5.6):
    """Dominant period/frequency of the rider's vertical bob during the gallop."""
    _seat(rows)
    t, v = _resample(rows, "seat_z", t0, t1)
    if t is None:
        t, v = _resample(rows, "rider_z", t0, t1)
    if t is None:
        t, v = _resample(rows, "mount_z", t0, t1)
    if t is None:
        return {}
    v = v - np.polyval(np.polyfit(t, v, 1), t)
    v = v - v.mean()
    win = np.hanning(len(v))
    spec = np.abs(np.fft.rfft(v * win))
    freqs = np.fft.rfftfreq(len(v), d=t[1] - t[0])
    band = (freqs >= 0.8) & (freqs <= 6.0)
    if not band.any():
        return {}
    i = np.argmax(spec[band])
    f = float(freqs[band][i])
    amplitude = float((v.max() - v.min()) / 2.0)
    rms = float(np.sqrt(np.mean(v * v)))
    return {"stride_hz": f, "stride_period_ms": 1000.0 / f if f else 0.0,
            "bob_amplitude_m": amplitude, "bob_rms_m": rms,
            "bob_samples": int(len(v))}


def _yaw_delta(rows, t0=6.5, t1=8.5, a_key="rider_yaw", b_key="mount_yaw"):
    vals = []
    for r in _sub(rows, t0, t1):
        a, b = r.get(a_key), r.get(b_key)
        if isinstance(a, (int, float)) and isinstance(b, (int, float)):
            d = (float(a) - float(b) + math.pi) % (2 * math.pi) - math.pi
            vals.append(math.degrees(d))
    if not vals:
        return {}
    return {"mean_deg": float(np.mean(vals)), "rms_deg": float(np.sqrt(np.mean(np.square(vals)))),
            "max_abs_deg": float(np.max(np.abs(vals))), "samples": len(vals)}


def _speed_mean(rows, t0, t1):
    sub = _sub(rows, t0, t1)
    vals = []
    prev = None
    for r in sub:
        x, y, t = r.get("mount_x"), r.get("mount_y"), r["t_ms"] / 1000.0
        if not all(isinstance(v, (int, float)) for v in (x, y)):
            x, y = r.get("player_x"), r.get("player_y")
        if all(isinstance(v, (int, float)) for v in (x, y)):
            if prev is not None:
                dt = t - prev[2]
                if dt > 1e-5:
                    vals.append((((x - prev[0]) ** 2 + (y - prev[1]) ** 2) ** 0.5) / dt)
            prev = (x, y, t)
    return float(np.mean(vals)) if vals else None


def _event(scenario, label):
    return next((e for e in scenario.get("events", []) if e.get("label") == label), None)


def compute(scenario, orig, bevy, bevy_label="bevy"):
    out = {"scenario": scenario["name"], "bevy_label": bevy_label, "original": {}, "bevy": {}}
    gallop = _event(scenario, "gallop")
    qw = _event(scenario, "q_w")
    for name, rows in (("original", orig), ("bevy", bevy)):
        if not rows:
            continue
        d = {}
        if scenario.get("mount"):
            t0 = (gallop["at"] + 1.0) if gallop else 1.0
            t1 = (gallop["until"] - 0.4) if gallop else 5.6
            d.update(_stride(rows, t0, t1))
            t0y, t1y = (qw["at"], qw["until"]) if qw else (6.5, 8.5)
            d.update(_yaw_delta(rows, t0y, t1y))
            bone = _yaw_delta(rows, t0y, t1y, "rider_bone_yaw", "mount_bone_yaw")
            d.update({"bone_" + key: value for key, value in bone.items()})
            d["gallop_mean_mps"] = _speed_mean(rows, t0, t1)
        d["rows"] = len(rows)
        d["duration_ms"] = rows[-1]["t_ms"] - rows[0]["t_ms"]
        d["fps_mean"] = (len(rows) - 1) / max(1e-9, d["duration_ms"] / 1000.0)
        out[name] = d
    return out
