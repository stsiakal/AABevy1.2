"""Side-by-side frame strip: hidden original client over Bevy offscreen frames."""

import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def _font(size=18):
    try:
        return ImageFont.truetype("arial.ttf", size)
    except Exception:
        return ImageFont.load_default()


def _nearest(items, t):
    return min(items, key=lambda x: abs(x[0] - t))


def make(out, scenario, orig, bevy, bevy_label="bevy"):
    meta_path = out / "original" / "meta.json"
    if not meta_path.exists():
        return
    meta = json.loads(meta_path.read_text(encoding="utf-8"))
    wall0 = meta["wall_start_ms"]
    orig_frames = [(f["wall_ms"] - wall0, out / "original" / f["file"])
                   for f in meta.get("frames", []) if (out / "original" / f["file"]).exists()]
    bev_frames = [(r["t_ms"], out / "bevy" / "frames" / ("b_%06d.png" % (int(r["frame"]) + 1)))
                  for r in bevy if r.get("frame") is not None]
    bev_frames = [(t, p) for t, p in bev_frames if p.exists()]
    if not orig_frames and not bev_frames:
        return
    end = max((e.get("until", e["at"]) for e in scenario["events"]), default=10.0)
    times = [end * i / 9.0 for i in range(10)]
    width = 620
    rows = []
    font = _font(16)
    for t in times:
        cells = []
        for label, frames in (("original 1.2", orig_frames), (bevy_label, bev_frames)):
            if frames:
                ft, path = _nearest(frames, t * 1000.0)
                try:
                    im = Image.open(path).convert("RGB")
                    h = max(1, round(im.height * width / im.width))
                    cells.append((label, ft / 1000.0, im.resize((width, h), Image.LANCZOS)))
                    continue
                except Exception:
                    pass
            cells.append((label, t, None))
        rows.append((t, cells))
    heights = []
    for _, cells in rows:
        heights.append(max((im.height for _, _, im in cells if im), default=360) + 28)
    canvas = Image.new("RGB", (width * 2 + 12, sum(heights)), (16, 16, 18))
    draw = ImageDraw.Draw(canvas)
    y = 0
    for (t, cells), h in zip(rows, heights):
        x = 0
        for label, shown_t, im in cells:
            draw.text((x + 6, y + 4), "%s  t=%.2fs" % (label, shown_t), fill=(235, 235, 235), font=font)
            if im:
                canvas.paste(im, (x + 2, y + 24))
            else:
                draw.rectangle([x + 2, y + 24, x + width, y + h - 4], outline=(90, 90, 90))
                draw.text((x + 12, y + 30), "(no frame)", fill=(160, 160, 160), font=font)
            x += width + 8
        y += h
    path = out / "strip.png"
    canvas.save(path)
    print("feel: strip ->", path, flush=True)
