#!/usr/bin/env python3
"""Monta a prancha com os oito estilos de papel de parede (escuro, acento ember) e dois no tema claro.

uso: wallpaper_sheet.py caminho/do/clios raiz-do-repo saida.png
"""
import os, pathlib, shutil, subprocess, sys, tempfile
from PIL import Image, ImageDraw

clios, repo, out = sys.argv[1], sys.argv[2], sys.argv[3]
STYLES = ["grade", "brilho", "aneis", "linhas", "blocos", "c-gigante", "diagonais", "solido"]
W, H, GAP = 480, 270, 14

def render(home, style):
    env = dict(os.environ, CLIOS_WALLPAPER_SIZE="960x540")
    base = [clios, "--root", repo, "--home", home]
    subprocess.run(base + ["wallpaper", "set", style], check=True, capture_output=True, env=env)
    path = subprocess.run(base + ["wallpaper", "path"], check=True, capture_output=True, text=True, env=env).stdout.strip()
    return Image.open(path).convert("RGB").resize((W, H), Image.LANCZOS)

with tempfile.TemporaryDirectory() as tmp:
    dark = pathlib.Path(tmp) / "dark"
    subprocess.run([clios, "--root", repo, "--home", str(dark), "theme", "set", "--mode", "dark", "--accent", "ember"], check=True, capture_output=True)
    light = pathlib.Path(tmp) / "light"
    subprocess.run([clios, "--root", repo, "--home", str(light), "theme", "set", "--mode", "light", "--accent", "azure"], check=True, capture_output=True)
    tiles = [render(str(dark), s) for s in STYLES]
    tiles += [render(str(light), "grade"), render(str(light), "c-gigante")]

    cols = 4
    rows = (len(tiles) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * W + (cols + 1) * GAP, rows * H + (rows + 1) * GAP), (24, 24, 24))
    for i, t in enumerate(tiles):
        x = GAP + (i % cols) * (W + GAP)
        y = GAP + (i // cols) * (H + GAP)
        sheet.paste(t, (x, y))
        ImageDraw.Draw(sheet).rectangle([x - 1, y - 1, x + W, y + H], outline=(60, 60, 60))
    sheet.save(out, optimize=True)
