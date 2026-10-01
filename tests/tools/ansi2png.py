#!/usr/bin/env python3
"""Converte a saída ANSI de 24 bits de `clios hub --snapshot` em PNG, usando o Chromium.

uso: ansi2png.py entrada.ansi saida.png [--font /caminho/GeistMono-Regular.ttf] [--bold ...] [--px 15]
"""
import argparse, html, re, sys, pathlib
from playwright.sync_api import sync_playwright

SGR = re.compile(r"\x1b\[([0-9;]*)m")

def parse(text):
    rows = []
    for line in text.split("\n"):
        if not line:
            continue
        cells, fg, bg, bold = [], None, None, False
        pos = 0
        for m in SGR.finditer(line):
            chunk = line[pos:m.start()]
            for ch in chunk:
                cells.append((ch, fg, bg, bold))
            pos = m.end()
            codes = [int(c) for c in m.group(1).split(";") if c != ""] or [0]
            i = 0
            while i < len(codes):
                c = codes[i]
                if c == 0:
                    fg = bg = None; bold = False
                elif c == 1:
                    bold = True
                elif c == 38 and codes[i+1] == 2:
                    fg = tuple(codes[i+2:i+5]); i += 4
                elif c == 48 and codes[i+1] == 2:
                    bg = tuple(codes[i+2:i+5]); i += 4
                i += 1
        for ch in line[pos:]:
            cells.append((ch, fg, bg, bold))
        rows.append(cells)
    return rows

def to_html(rows, font, bold_font, px, pad):
    page_bg = next((c[2] for r in rows for c in r if c[2]), (0, 0, 0))
    css = lambda c: "rgb(%d,%d,%d)" % c
    out = []
    for cells in rows:
        spans, cur, key = [], "", None
        for ch, fg, bg, bold in cells:
            k = (fg, bg, bold)
            if k != key:
                if cur:
                    spans.append((key, cur))
                cur, key = "", k
            cur += ch
        if cur:
            spans.append((key, cur))
        line = ""
        for (fg, bg, bold), text in spans:
            style = ""
            if fg: style += "color:%s;" % css(fg)
            if bg: style += "background:%s;" % css(bg)
            if bold: style += "font-weight:700;"
            body = html.escape(text).replace("\ue0b6", '<i class="cl"></i>').replace("\ue0b4", '<i class="cr"></i>')
            if "▀" in body:
                # meio-bloco: dois pixels por célula, preenchendo a altura toda da linha (sem a fresta do line-height)
                half = '<i class="hb" style="background:linear-gradient(%s 50%%,%s 50%%)"></i>' % (css(fg or (0, 0, 0)), css(bg or (0, 0, 0)))
                body = body.replace("▀", half)
            line += '<span style="%s">%s</span>' % (style, body)
        out.append('<div class="r">%s</div>' % line)
    import base64
    def face(path, weight):
        data = base64.b64encode(pathlib.Path(path).read_bytes()).decode()
        return "@font-face{font-family:'GM';font-weight:%d;src:url(data:font/ttf;base64,%s);}" % (weight, data)
    ff = ""
    if font:
        ff += face(font, 400)
    if bold_font:
        ff += face(bold_font, 700)
    return f"""<!doctype html><meta charset=utf-8><style>{ff}
    html,body{{margin:0;background:rgb({page_bg[0]},{page_bg[1]},{page_bg[2]})}}
    #t{{padding:{pad}px;display:inline-block;background:rgb({page_bg[0]},{page_bg[1]},{page_bg[2]});font-family:'GM','DejaVu Sans Mono',monospace;font-size:{px}px;line-height:1.35;white-space:pre}}
    .r{{height:1.35em}}
    .cl,.cr{{display:inline-block;width:1ch;height:1.35em;vertical-align:top;background:currentColor}}
    .hb{{display:inline-block;width:1ch;height:1.35em;vertical-align:top}}
    .cl{{border-radius:100% 0 0 100% / 50% 0 0 50%}} .cr{{border-radius:0 100% 100% 0 / 0 50% 50% 0}}</style><div id=t>{''.join(out)}</div>"""

ap = argparse.ArgumentParser()
ap.add_argument("inp"); ap.add_argument("out")
ap.add_argument("--font"); ap.add_argument("--bold"); ap.add_argument("--px", type=int, default=15)
ap.add_argument("--pad", type=int, default=0)
a = ap.parse_args()
text = pathlib.Path(a.inp).read_text()
doc = to_html(parse(text), a.font, a.bold, a.px, a.pad)
with sync_playwright() as p:
    b = p.chromium.launch(executable_path="/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args=["--no-sandbox"])
    pg = b.new_page(device_scale_factor=2)
    pg.set_content(doc); pg.wait_for_timeout(300)
    pg.locator("#t").screenshot(path=a.out)
    b.close()
print("ok", a.out)
