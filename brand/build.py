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

# Geometria da marca, em grade de 64. Mantida igual em config/quickshell/components/Mark.qml
# (um teste confere que os dois caminhos são o mesmo).
#   corpo   = quadrado 64x64 menos a boca (x 20..64, y 20..44): um "c" de traço uniforme (20)
#   cursor  = bloco 12x24 dentro da boca, centrado na vertical, como um cursor de terminal esperando
# Cantos levemente arredondados, como o resto da interface: 6 nos externos, 3 nas pontas dos braços
# e 2 nos cantos internos da boca. Um polígono só: retângulos adjacentes deixam fresta de antialiasing.
VERTS = [(0, 0, 6), (64, 0, 6), (64, 20, 3), (20, 20, 2), (20, 44, 2), (64, 44, 3), (64, 64, 6), (0, 64, 6)]
CURSOR = (34, 20, 12, 24)
CURSOR_R = 2


def rounded_path(verts):
    """Caminho SVG de um polígono com raio por vértice (arcos tangentes às arestas)."""
    n = len(verts)
    pts = []
    for i, (x, y, r) in enumerate(verts):
        px, py, _ = verts[i - 1]
        nx, ny, _ = verts[(i + 1) % n]
        d1 = (px - x, py - y)
        d2 = (nx - x, ny - y)
        l1, l2 = (d1[0] ** 2 + d1[1] ** 2) ** 0.5, (d2[0] ** 2 + d2[1] ** 2) ** 0.5
        a = (x + d1[0] / l1 * r, y + d1[1] / l1 * r)
        b = (x + d2[0] / l2 * r, y + d2[1] / l2 * r)
        # cross > 0: curva para o sentido horário (sweep = 1) em coordenadas com y para baixo
        cross = (x - px) * (ny - y) - (y - py) * (nx - x)
        pts.append((a, b, r, 1 if cross > 0 else 0))
    f = lambda v: f"{v:g}"
    d = f"M{f(pts[0][0][0])} {f(pts[0][0][1])}"
    for i, (a, b, r, sw) in enumerate(pts):
        if i:
            d += f"L{f(a[0])} {f(a[1])}"
        if r:
            d += f"A{f(r)} {f(r)} 0 0 {sw} {f(b[0])} {f(b[1])}"
    return d + "Z"


BODY = rounded_path(VERTS)


def mark(body, cursor, bg=None, pad=0):
    size = 64 + pad * 2
    x, y, w, h = CURSOR
    inner = f'<path d="{BODY}" fill="{body}"/><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{CURSOR_R}" fill="{cursor}"/>'
    back = f'<rect x="{-pad}" y="{-pad}" width="{size}" height="{size}" rx="{size * 0.22:g}" fill="{bg}"/>' if bg else ""
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
        f'<rect x="{cx:.1f}" y="{cy:.1f}" width="{cw:.1f}" height="{ch_h:.1f}" rx="{height * 0.045:.1f}" fill="{cursor}"/></svg>\n'
    )


def main():
    if "--path" in sys.argv:  # usado pelos testes
        print(BODY)
        return
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
