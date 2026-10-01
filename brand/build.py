#!/usr/bin/env python3
"""Gera os SVGs da marca. A marca em si é geometria pura; o wordmark usa contornos reais da
Geist Mono (OFL), então o SVG final não depende da fonte estar instalada.

uso: brand/build.py /caminho/para/GeistMono-Medium.ttf
requer: pip install fonttools
"""
import pathlib, sys
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

OUT = pathlib.Path(__file__).resolve().parent

# Paletas (tokens/tokens.toml). A marca só usa neutro + acento.
INK, PAPER = "#0A0A0A", "#F5F5F5"
EMBER_D, EMBER_L = "#FF5A1F", "#CC3E00"

# Geometria da marca, em grade de 64. Mantida igual em config/quickshell/components/Mark.qml.
#   corpo   = quadrado 64x64 menos a boca (x 20..64, y 20..44): um "c" de traço uniforme (20)
#   cursor  = bloco 12x24 dentro da boca, centrado na vertical, como um cursor de terminal esperando
# Um polígono só: retângulos adjacentes deixam uma fresta de antialiasing.
BODY = "M0 0H64V20H20V44H64V64H0Z"
CURSOR = (34, 20, 12, 24)


def mark(body, cursor, bg=None, pad=0):
    size = 64 + pad * 2
    x, y, w, h = CURSOR
    inner = f'<path d="{BODY}" fill="{body}"/><rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{cursor}"/>'
    back = f'<rect x="{-pad}" y="{-pad}" width="{size}" height="{size}" fill="{bg}"/>' if bg else ""
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{-pad} {-pad} {size} {size}" role="img" aria-label="clios">'
        f"{back}{inner}</svg>\n"
    )


def wordmark(font_path, body, cursor, bg=None, height=64):
    """`clios` em contornos + o cursor de bloco depois da última letra."""
    font = TTFont(font_path)
    gs = font.getGlyphSet()
    cmap = font.getBestCmap()
    upm = font["head"].unitsPerEm
    cap = font["OS/2"].sxHeight or int(upm * 0.55)  # altura do x: as letras são todas minúsculas
    ascender = int(upm * 0.74)                       # topo do "l"
    scale = height / (ascender * 1.0)
    adv = gs[cmap[ord("c")]].width
    letters = "clios"
    paths = []
    for i, ch in enumerate(letters):
        pen = SVGPathPen(gs)
        # y invertido: o SVG cresce para baixo; baseline em y = ascender*scale
        tp = TransformPen(pen, (scale, 0, 0, -scale, i * adv * scale, ascender * scale))
        gs[cmap[ord(ch)]].draw(tp)
        paths.append(pen.getCommands())
    text_w = len(letters) * adv * scale
    # cursor: um bloco do tamanho do corpo do "x", colado depois do texto
    cw = adv * scale * 0.5
    ch_h = cap * scale * 1.0
    cx = text_w + adv * scale * 0.12
    cy = ascender * scale - ch_h
    total_w = cx + cw
    d = " ".join(paths)
    back = f'<rect width="{total_w:.1f}" height="{height}" fill="{bg}"/>' if bg else ""
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total_w:.1f} {height}" role="img" aria-label="clios">'
        f'{back}<path d="{d}" fill="{body}"/>'
        f'<rect x="{cx:.1f}" y="{cy:.1f}" width="{cw:.1f}" height="{ch_h:.1f}" fill="{cursor}"/></svg>\n'
    )


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    font = sys.argv[1]
    files = {
        "mark.svg": mark(PAPER, EMBER_D),                       # para fundo escuro
        "mark-on-light.svg": mark(INK, EMBER_L),                # para fundo claro
        "mark-mono-light.svg": mark(PAPER, PAPER),              # uma cor, fundo escuro
        "mark-mono-dark.svg": mark(INK, INK),                   # uma cor, fundo claro
        "mark-tile.svg": mark(PAPER, EMBER_D, bg="#000000", pad=24),  # avatar / ícone quadrado
        "wordmark.svg": wordmark(font, PAPER, EMBER_D),
        "wordmark-on-light.svg": wordmark(font, INK, EMBER_L),
    }
    for name, svg in files.items():
        (OUT / name).write_text(svg)
        print("escrito", name, f"({len(svg)} bytes)")


if __name__ == "__main__":
    main()
