#!/usr/bin/env python3
"""Gera site/img/og.png (1200x630), a imagem que aparece quando o link do site é compartilhado.
uso: site/og.py [--font /caminho/GeistMono-Bold.ttf]   (requer playwright com chromium)"""
import base64, pathlib, re, sys
from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parent.parent
wm = (ROOT / "brand/wordmark.svg").read_text()
wm = re.sub(r"<svg [^>]*>", '<svg viewBox="0 0 291.6 64" width="520" xmlns="http://www.w3.org/2000/svg">', wm, count=1)
font = ""
if "--font" in sys.argv:
    data = base64.b64encode(pathlib.Path(sys.argv[sys.argv.index("--font") + 1]).read_bytes()).decode()
    font = "@font-face{font-family:'GM';font-weight:700;src:url(data:font/ttf;base64,%s)}" % data
page = f"""<!doctype html><meta charset=utf-8><style>{font}
html,body{{margin:0;background:#000}}
.c{{width:1200px;height:630px;box-sizing:border-box;padding:72px 80px;background:#000;color:#F5F5F5;font-family:'GM','Geist Mono',monospace;position:relative;overflow:hidden}}
.c::before{{content:"";position:absolute;right:-120px;bottom:-160px;width:560px;height:560px;border-radius:50%;background:radial-gradient(circle,#FF5A1F55,transparent 66%)}}
h1{{font-size:54px;line-height:1.12;margin:44px 0 0;max-width:820px;font-weight:700;letter-spacing:-.02em}}
p{{font-size:24px;color:#A8A8A8;margin:28px 0 0;max-width:820px;line-height:1.5}}
.tag{{position:absolute;left:80px;bottom:60px;font-size:20px;color:#6B6B6B}}
</style><div class=c>{wm}<h1>Um desktop Linux feito para o terminal.</h1><p>Arch, Hyprland e Quickshell. Preto e branco, um acento seu, uma fonte só.</p><div class=tag>kitsuneislife.github.io/clios</div></div>"""
with sync_playwright() as p:
    b = p.chromium.launch(executable_path="/opt/pw-browsers/chromium-1194/chrome-linux/chrome", args=["--no-sandbox"])
    pg = b.new_page(viewport={"width": 1200, "height": 630})
    pg.set_content(page)
    pg.wait_for_timeout(400)
    (ROOT / "site/img").mkdir(exist_ok=True)
    pg.screenshot(path=str(ROOT / "site/img/og.png"))
    b.close()
print("site/img/og.png")
