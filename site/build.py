#!/usr/bin/env python3
"""Gera site/index.html: o texto e o desenho vêm de site/page.html; os números, o catálogo de apps, os atalhos,
os acentos e o changelog vêm dos arquivos do próprio repositório. A página não tem como envelhecer em relação a eles.

uso: site/build.py            escreve site/index.html
     site/build.py --check    sai com erro se site/index.html não bate com o que seria gerado (o CI usa)
"""
import html, json, re, shutil, sys, tomllib
from pathlib import Path

SITE = Path(__file__).resolve().parent
ROOT = SITE.parent

# A ordem e os nomes das categorias são os de crates/clios/src/catalog.rs (um teste confere).
CATEGORIES = [
    ("arquivos", "arquivos"), ("codigo", "código"), ("sistema", "sistema"), ("rede", "rede"), ("midia", "mídia"),
    ("ler", "ler"), ("falar", "conversar"), ("produtividade", "produtividade"), ("pacotes", "pacotes"), ("diversao", "diversão"),
]


def esc(s):
    return html.escape(s, quote=True)


def inline(text):
    """Markdown mínimo: `código`, **negrito** e [texto](url). O resto é texto."""
    out = esc(text)
    out = re.sub(r"`([^`]+)`", r"<code>\1</code>", out)
    out = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", out)
    out = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", r'<a href="\2">\1</a>', out)
    return out


def load_toml(path):
    return tomllib.loads((ROOT / path).read_text())


def tokens():
    t = load_toml("tokens/tokens.toml")
    return {
        "dark": t["mode"]["dark"],
        "light": t["mode"]["light"],
        "accents": {k: {"dark": v["dark"], "light": v["light"]} for k, v in t["accent"].items()},
    }


def catalog():
    tui = load_toml("config/clios/hub.toml")["tui"]
    known = {c for c, _ in CATEGORIES}
    bad = {t["category"] for t in tui} - known
    assert not bad, f"categorias que o site não conhece: {bad}"
    return tui


def render_apps(tui):
    names = dict(CATEGORIES)
    out = []
    for cat, label in CATEGORIES:
        items = [t for t in tui if t["category"] == cat]
        cards = []
        for t in items:
            own = t["pkg"] == "clios"
            badge = "do CLIOS" if own else ("vem instalado" if t.get("tier") == "core" else "instala com +")
            cls = "b own" if own else ("b core" if t.get("tier") == "core" else "b")
            prog = t.get("bin") or t["cmd"][0]
            cards.append(
                f'<article class="app" data-cat="{cat}" data-q="{esc((t["name"] + " " + prog + " " + t["desc"] + " " + t.get("keywords", "")).lower())}">'
                f'<header><h4>{esc(t["name"])}</h4><span class="{cls}">{badge}</span></header>'
                f'<p class="d">{esc(t["desc"])}</p>'
                f'<p class="t">{esc(t.get("tip", ""))}</p>'
                f'<footer><code>{esc(prog if not own else "clios")}</code><span class="job">faz: {esc(t["job"].replace("-", " "))}</span></footer>'
                "</article>"
            )
        out.append(f'<div class="cat" data-cat="{cat}"><h3>{esc(label)} <span class="n">{len(items)}</span></h3><div class="apps">{"".join(cards)}</div></div>')
    return "\n".join(out), names


def keycaps(keys):
    parts = [p.strip() for p in keys.split(" + ")]
    return '<span class="plus"> + </span>'.join(f"<kbd>{esc(p)}</kbd>" for p in parts)


def render_keys():
    groups = load_toml("config/clios/keys.toml")["group"]
    out = []
    n = 0
    for g in groups:
        rows = []
        for k in g["key"]:
            n += 1
            rows.append(f'<li data-q="{esc((k["keys"] + " " + k["what"] + " " + g["name"]).lower())}"><span class="k">{keycaps(k["keys"])}</span><span class="w">{esc(k["what"])}</span></li>')
        out.append(f'<div class="kgroup"><h3>{esc(g["name"])}</h3><p>{esc(g["blurb"])}</p><ul>{"".join(rows)}</ul></div>')
    return "\n".join(out), n


def render_changelog():
    text = (ROOT / "CHANGELOG.md").read_text()
    versions = re.split(r"(?m)^## ", text)[1:]
    out = []
    for i, v in enumerate(versions):
        head, _, body = v.partition("\n")
        parts, in_list = [], False
        for line in body.splitlines():
            line = line.rstrip()
            m = re.fullmatch(r"\*\*(.+)\*\*", line)
            if line.startswith("- "):
                if not in_list:
                    parts.append("<ul>")
                    in_list = True
                parts.append(f"<li>{inline(line[2:])}</li>")
                continue
            if in_list:
                parts.append("</ul>")
                in_list = False
            if m:
                parts.append(f"<h4>{esc(m.group(1))}</h4>")
            elif line:
                parts.append(f"<p>{inline(line)}</p>")
        if in_list:
            parts.append("</ul>")
        out.append(f'<details class="ver"{" open" if i == 0 else ""}><summary><span class="v">{esc(head.strip())}</span></summary>{"".join(parts)}</details>')
    return "\n".join(out), versions[0].split("\n")[0].strip()


def stats(tui, n_keys_curated):
    readme = (ROOT / "README.md").read_text()
    tests = re.search(r"\((\d+) testes de Rust", readme)
    keys_md = (ROOT / "docs/KEYS.md").read_text()
    binds = len(re.findall(r"(?m)^\| `", keys_md))
    return {
        "apps": len([t for t in tui if t["pkg"] != "clios"]),
        "own": len([t for t in tui if t["pkg"] == "clios"]),
        "core": len([t for t in tui if t.get("tier") == "core" and t["pkg"] != "clios"]),
        "toys": len([t for t in tui if t.get("saver")]),
        "binds": binds,
        "curated": n_keys_curated,
        "tests": int(tests.group(1)) if tests else 0,
    }


# Imagens da documentação que o site reaproveita (o site/img/toys e o fetch.png saem de tests/tools/toyshots.sh).
DOC_IMAGES = ["hub.png", "hub-light.png", "welcome-inicio.png", "welcome-sistema.png", "shell-dark.png", "wallpaper-picker.png", "wallpapers.png"]


def copy_images():
    (SITE / "img").mkdir(exist_ok=True)
    for name in DOC_IMAGES:
        shutil.copyfile(ROOT / "docs/img" / name, SITE / "img" / name)


def build():
    page = (SITE / "page.html").read_text()
    tui = catalog()
    apps_html, _ = render_apps(tui)
    keys_html, n_curated = render_keys()
    log_html, version = render_changelog()
    st = stats(tui, n_curated)
    tk = tokens()
    toys = [{"id": t["id"], "name": t["name"], "cmd": t["saver"], "desc": t["desc"]} for t in tui if t.get("saver")]
    from struct import unpack
    png = (SITE / "img/toys/bonsai.png").read_bytes()
    toyimg = list(unpack(">II", png[16:24]))  # largura e altura, do cabeçalho do PNG
    data = {"tokens": tk, "stats": st, "toys": toys, "version": version, "toyimg": toyimg}

    wordmark = (ROOT / "brand/wordmark.svg").read_text()
    wordmark = re.sub(r'<svg [^>]*>', '<svg class="wm" viewBox="0 0 291.6 64" role="img" aria-label="clios" xmlns="http://www.w3.org/2000/svg">', wordmark, count=1)
    wordmark = wordmark.replace(' fill="#F5F5F5"', "").replace(' fill="#FF5A1F"', ' class="cur"')

    subs = {
        "<!--@apps-->": apps_html,
        "<!--@keys-->": keys_html,
        "<!--@changelog-->": log_html,
        "<!--@wordmark-->": wordmark,
        "/*@data*/": json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c"),
    }
    for k, v in subs.items():
        assert k in page, f"falta {k} em page.html"
        page = page.replace(k, v)
    for k, v in {**st, "version": version}.items():
        page = page.replace("{{" + k + "}}", str(v))
    for ref in set(re.findall(r'(?:src|href)="(img/[^"$]+)"', page)):
        assert (SITE / ref).exists(), f"a página usa {ref}, que não existe (rode tests/tools/toyshots.sh ou site/og.py)"
    for t in toys:
        assert (SITE / "img/toys" / (t["id"] + ".png")).exists() or t["id"] in ("genact",), f"falta site/img/toys/{t['id']}.png"
    left = re.findall(r"\{\{(\w+)\}\}", page)
    assert not left, f"marcadores sem valor: {left}"
    return page


if __name__ == "__main__":
    out = build()
    target = SITE / "index.html"
    if "--check" in sys.argv:
        if not target.exists() or target.read_text() != out:
            sys.exit("site/index.html está desatualizado: rode site/build.py")
        print("site/index.html em dia")
    else:
        copy_images()
        target.write_text(out)
        print(f"site/index.html: {len(out) // 1024} KiB")
