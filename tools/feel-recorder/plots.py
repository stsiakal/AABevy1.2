"""Overlay plots for the feel recorder (matplotlib, headless Agg)."""

from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


def _series(rows, key):
    xs, ys = [], []
    for r in rows:
        v = r.get(key)
        if isinstance(v, (int, float)):
            xs.append(r["t_ms"] / 1000.0)
            ys.append(float(v))
    return xs, ys


def _smooth(xs, ys, width=0.25):
    """Resample to 50 Hz and box-average; position samples arrive at packet rate, so the
    raw per-row derivative is a spike train."""
    if len(xs) < 3:
        return xs, ys
    x = np.asarray(xs, dtype=float)
    y = np.asarray(ys, dtype=float)
    order = np.argsort(x)
    x, y = x[order], y[order]
    if x[-1] - x[0] < width:
        return xs, ys
    grid = np.arange(x[0], x[-1], 1.0 / 50.0)
    yy = np.interp(grid, x, y)
    win = max(1, int(round(width * 50.0)))
    kernel = np.ones(win) / win
    return grid.tolist(), np.convolve(yy, kernel, mode="same").tolist()


def _speed(rows, prefix):
    xs, ys = [], []
    prev = None
    for r in rows:
        vx, vy = r.get(prefix + "_vx"), r.get(prefix + "_vy")
        if isinstance(vx, (int, float)) and isinstance(vy, (int, float)) and (vx * vx + vy * vy) > 1e-6:
            xs.append(r["t_ms"] / 1000.0)
            ys.append((vx * vx + vy * vy) ** 0.5)
            prev = None
            continue
        x, y, t = r.get(prefix + "_x"), r.get(prefix + "_y"), r["t_ms"] / 1000.0
        if all(isinstance(v, (int, float)) for v in (x, y)):
            if prev is not None:
                dt = t - prev[2]
                d = (((x - prev[0]) ** 2 + (y - prev[1]) ** 2) ** 0.5) if dt > 1e-5 else 0.0
                if dt > 1e-5 and d / dt < 25.0:
                    xs.append(t)
                    ys.append(d / dt)
            prev = (x, y, t)
    return _smooth(xs, ys) if xs else (xs, ys)


def _plot(out, name, title, ylabel, series):
    fig, ax = plt.subplots(figsize=(11, 4.2), dpi=110)
    plotted = False
    for label, xs, ys, style in series:
        if xs:
            ax.plot(xs, ys, style, label=label, linewidth=1.2)
            plotted = True
    if not plotted:
        plt.close(fig)
        return
    ax.set_title(title)
    ax.set_xlabel("scenario time (s)")
    ax.set_ylabel(ylabel)
    ax.grid(True, alpha=0.25)
    ax.legend(loc="best", fontsize=8)
    fig.tight_layout()
    fig.savefig(out / name)
    plt.close(fig)


def make_all(out, scenario, orig, bevy, bevy_label="bevy"):
    plots = out / "plots"
    plots.mkdir(exist_ok=True)
    pairs = {"original": orig, bevy_label: bevy}
    styles = {"original": "-"}
    styles[bevy_label] = "--"

    def one(name, title, ylabel, fn):
        series = []
        for label, rows in pairs.items():
            xs, ys = fn(rows)
            series.append((label, xs, ys, styles[label]))
        _plot(plots, name, title, ylabel, series)

    one("speed.png", "Ground speed", "m/s", lambda r: _speed(r, "player"))
    one("mount_speed.png", "Mount ground speed", "m/s", lambda r: _speed(r, "mount"))
    for key in ("player_x", "player_y", "player_z", "mount_x", "mount_y", "mount_z",
                "rider_x", "rider_y", "rider_z", "cam_x", "cam_y", "cam_z",
                "cam_dx", "cam_dy", "cam_dz",
                "server_x", "server_y", "server_z",
                "server_mount_x", "server_mount_y", "server_mount_z"):
        one(key + ".png", key.replace("_", " "), "m", lambda r, k=key: _series(r, k))
    for key in ("player_vx", "player_vy", "player_vz", "mount_vx", "mount_vy", "mount_vz"):
        one(key + ".png", key.replace("_", " "), "m/s", lambda r, k=key: _series(r, k))
    for key in ("player_yaw", "mount_yaw", "mount_pitch", "rider_yaw", "rider_bone_yaw",
                "mount_bone_yaw", "cam_yaw", "cam_pitch"):
        one(key + ".png", key.replace("_", " "), "degrees", lambda r, k=key: _deg(r, k))
    for key in ("player_clip_ms", "mount_clip_ms", "rider_clip_ms"):
        one(key + ".png", key.replace("_", " "), "ms", lambda r, k=key: _series(r, k))
    one("position.png", "Position x/y/z", "m", lambda r: _series(r, "player_y"))
    one("yaw.png", "Player / mount yaw", "degrees", lambda r: _deg(r, "player_yaw"))
    one("mount_yaw.png", "Mount yaw", "degrees", lambda r: _deg(r, "mount_yaw"))
    one("rider_bob.png", "Rider pelvis height above mount root", "m", _seat)
    one("rider_bob_centered.png", "Rider bob (mean removed, 1.0-5.6 s)", "m",
        _seat_centered)
    one("rider_vs_mount.png", "Rider minus mount yaw (entity)", "degrees", _delta_yaw)
    one("rider_bone_vs_mount.png", "Rider pelvis bone minus mount root bone yaw", "degrees",
        lambda r: _delta_yaw(r, "rider_bone_yaw", "mount_bone_yaw"))
    one("camera.png", "Camera y offset from player", "m", lambda r: _offset(r, "cam_y", "player_y"))
    one("camera_offset_x.png", "Camera x offset from player", "m", lambda r: _offset(r, "cam_x", "player_x"))
    one("camera_offset_z.png", "Camera z offset from player", "m", lambda r: _offset(r, "cam_z", "player_z"))
    one("camera_yaw.png", "Camera heading", "degrees", lambda r: _deg(r, "cam_yaw"))
    one("camera_pitch.png", "Camera pitch", "degrees", lambda r: _deg(r, "cam_pitch"))
    one("clip_time.png", "Player clip time", "ms", lambda r: _series(r, "player_clip_ms"))

    # combined overview
    fig, axes = plt.subplots(5, 1, figsize=(12, 13), dpi=110, sharex=True)
    for label, rows in pairs.items():
        s = styles[label]
        xs, ys = _speed(rows, "player")
        axes[0].plot(xs, ys, s, label=label, linewidth=1.1)
        xs, ys = _deg(rows, "player_yaw")
        axes[1].plot(xs, ys, s, label=label + " player", linewidth=1.0)
        xs, ys = _deg(rows, "mount_yaw")
        axes[1].plot(xs, ys, s, label=label + " mount", linewidth=0.9, alpha=0.7)
        xs, ys = _seat_centered(rows)
        axes[2].plot(xs, ys, s, label=label, linewidth=1.0)
        xs, ys = _delta_yaw(rows)
        axes[3].plot(xs, ys, s, label=label, linewidth=1.0)
        xs, ys = _series(rows, "cam_y")
        axes[4].plot(xs, ys, s, label=label, linewidth=1.0)
    for ax, title in zip(axes, ["speed (m/s)", "yaw (deg)", "rider bob, mean removed (m)", "rider-mount yaw (deg)", "camera y (m)"]):
        ax.set_title(title, fontsize=9)
        ax.grid(True, alpha=0.25)
        ax.legend(fontsize=7, loc="best")
    axes[-1].set_xlabel("scenario time (s)")
    fig.tight_layout()
    fig.savefig(plots / "overview.png")
    plt.close(fig)


def _deg(rows, key):
    import math
    xs, ys = [], []
    last = None
    for r in rows:
        v = r.get(key)
        if isinstance(v, (int, float)):
            d = math.degrees(float(v))
            if last is not None:
                while d - last > 180.0:
                    d -= 360.0
                while d - last < -180.0:
                    d += 360.0
            xs.append(r["t_ms"] / 1000.0)
            ys.append(d)
            last = d
    return xs, ys


def _offset(rows, a, b):
    xs, ys = [], []
    for r in rows:
        x, y = r.get(a), r.get(b)
        if isinstance(x, (int, float)) and isinstance(y, (int, float)):
            xs.append(r["t_ms"] / 1000.0)
            ys.append(float(x) - float(y))
    return xs, ys


def _seat(rows):
    xs, ys = [], []
    for r in rows:
        rz, mz = r.get("rider_z"), r.get("mount_z")
        if isinstance(rz, (int, float)) and isinstance(mz, (int, float)):
            xs.append(r["t_ms"] / 1000.0)
            ys.append(float(rz) - float(mz))
    return xs, ys


def _seat_centered(rows, t0=1.0, t1=5.6):
    xs, ys = _seat(rows)
    window = [y for x, y in zip(xs, ys) if t0 <= x <= t1]
    if not window:
        return xs, ys
    mean = sum(window) / len(window)
    return xs, [y - mean for y in ys]


def _detrended(rows, key, t0, t1):
    xs, ys = _series(rows, key)
    window = [y for x, y in zip(xs, ys) if t0 <= x <= t1]
    if not window:
        return xs, ys
    mean = sum(window) / len(window)
    return xs, [y - mean for y in ys]


def _delta_yaw(rows, a_key="rider_yaw", b_key="mount_yaw"):
    import math
    xs, ys = [], []
    for r in rows:
        a, b = r.get(a_key), r.get(b_key)
        if isinstance(a, (int, float)) and isinstance(b, (int, float)):
            d = (float(a) - float(b) + math.pi) % (2 * math.pi) - math.pi
            xs.append(r["t_ms"] / 1000.0)
            ys.append(math.degrees(d))
    return xs, ys
