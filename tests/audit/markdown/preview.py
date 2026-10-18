#!/usr/bin/env python3
"""Rasterize captured public-renderer SVGs and make labeled contact sheets.

Pillow is only required for this evidence tool, not Markie or run.py:
  python3 -m pip install --target .amp/in/markdown-python pillow
  PYTHONPATH=.amp/in/markdown-python python3 tests/audit/markdown/preview.py
"""
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('audit', HERE / 'run.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)
if len(sys.argv) != 2:
    raise SystemExit('Usage: preview.py ARTIFACT_BATCH_DIRECTORY')
OUT = audit.ROOT / sys.argv[1]
files = sorted(OUT.glob('MD-*/*.svg'))
result = subprocess.run([str(audit.ROOT / '.amp/in/audit/markdown-public-probe'), '--raster', *map(str, files)], env=audit.ENV, capture_output=True, text=True, cwd=audit.ROOT)
(OUT / 'raster.log').write_text(result.stdout + result.stderr)
rows = json.loads((OUT / 'results.json').read_text())
rows = [r for r in rows if (OUT / r['id'] / 'checks.json').exists()]
font = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', 14)
small = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', 12)
panels = []
for row in rows:
    images = []
    for variant, limit in [('light-default', 600), ('light-narrow', 360), ('dark-narrow', 360)]:
        p = OUT / row['id'] / (variant + '.png')
        if p.exists():
            image = Image.open(p).convert('RGB')
            if image.width > limit:
                image = image.resize((limit, max(1, round(image.height * limit / image.width))))
            # Preserve full PNGs alongside sheets; cap extreme cases only in the
            # contact sheet and label that crop explicitly.
            cropped = image.height > 500
            images.append((variant, image.crop((0, 0, image.width, min(image.height, 500))), cropped))
    height = max([im.height for _, im, _ in images] + [70]) + 55
    panel = Image.new('RGB', (1360, height), '#e5e7eb')
    draw = ImageDraw.Draw(panel)
    color = '#b91c1c' if row['status'] == 'failed' else '#166534'
    draw.text((10, 3), f"{row['id']}  {row['feature']}  [{row['status']}]", font=font, fill=color)
    for x, (variant, image, cropped) in zip([10, 620, 990], images):
        draw.text((x, 25), variant + (' (top 500px; full PNG saved)' if cropped else ''), font=small, fill='black')
        panel.paste(image, (x, 48))
    panels.append(panel)
for start in range(0, len(panels), 7):
    batch = panels[start:start + 7]
    image = Image.new('RGB', (1360, sum(p.height for p in batch) + len(batch) * 8), 'white')
    y = 0
    for panel in batch:
        image.paste(panel, (0, y))
        y += panel.height + 8
    path = OUT / f'contact-{start // 7 + 1:02}.png'
    image.save(path)
    print(path.relative_to(audit.ROOT), flush=True)
