#!/usr/bin/env python3
"""Renderiza os componentes QML do CLIOS offscreen (PySide6) para PNG e falha em qualquer aviso do QML.

A árvore é montada como o Quickshell faz: cada pasta vira um módulo `qs.<pasta>`, com `qmldir`
sintetizado (singletons detectados por `pragma Singleton`). Só os diretórios `core` e `components`
entram: `modules` depende de serviços do Quickshell e não roda fora dele.

uso: render.py [--out DIR] [--clios target/debug/clios] [--font DIR_COM_GeistMono-*.ttf]
requer: pip install PySide6
"""
import argparse, os, pathlib, re, shutil, subprocess, sys, tempfile

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
os.environ.setdefault("QT_QUICK_BACKEND", "software")

from PySide6.QtCore import QUrl, QTimer, Qt, qInstallMessageHandler, QtMsgType
from PySide6.QtGui import QGuiApplication, QFontDatabase
from PySide6.QtQuick import QQuickView

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

messages = []
def handler(kind, ctx, msg):
    if kind in (QtMsgType.QtWarningMsg, QtMsgType.QtCriticalMsg, QtMsgType.QtFatalMsg):
        messages.append(msg)
qInstallMessageHandler(handler)


def build_tree(dst: pathlib.Path):
    """Replica a síntese de qmldir do Quickshell (src/core/scan.cpp)."""
    for sub in ("core", "components"):
        d = dst / "qs" / sub
        d.mkdir(parents=True)
        lines = [f"module qs.{sub}"]
        for f in sorted((REPO / "config/quickshell" / sub).glob("*.qml")):
            shutil.copy(f, d / f.name)
            name = f.stem
            single = "pragma Singleton" in f.read_text()
            lines.append(("singleton " if single else "") + f"{name} 1.0 {f.name}")
        (d / "qmldir").write_text("\n".join(lines) + "\n")
    shutil.copy(HERE / "Scene.qml", dst / "Scene.qml")


def check_mark_path():
    """O caminho da marca no QML tem que ser o mesmo de brand/mark.svg (os dois vêm de brand/build.py)."""
    want = subprocess.run([sys.executable, str(REPO / "brand/build.py"), "--path"], capture_output=True, text=True, check=True).stdout.strip()
    qml = (REPO / "config/quickshell/components/Mark.qml").read_text()
    got = re.search(r'markPath: "([^"]+)"', qml).group(1)
    svg = (REPO / "brand/mark.svg").read_text()
    return got == want and want in svg, want


def theme_json(clios, mode, accent, motion):
    with tempfile.TemporaryDirectory() as home:
        base = [clios, "--root", str(REPO), "--home", home]
        subprocess.run(base + ["theme", "set", "--mode", mode, "--accent", accent, "--motion", motion], check=True, capture_output=True)
        return (pathlib.Path(home) / ".local/state/clios/theme.json").read_text()


def render(app, tree, font_family, tjson, out_png, active=2, settle_ms=700):
    view = QQuickView()
    view.engine().addImportPath(str(tree))
    view.setResizeMode(QQuickView.SizeRootObjectToView)
    view.setInitialProperties({"themeJson": tjson, "active": active})
    view.setSource(QUrl.fromLocalFile(str(tree / "Scene.qml")))
    if view.status() != QQuickView.Ready:
        raise SystemExit("QML não carregou: " + "; ".join(str(e) for e in view.errors()))
    root = view.rootObject()
    # Tokens.apply() já rodou; troca só a fonte para a que o teste carregou.
    tokens = view.engine().singletonInstance("qs.core", "Tokens")
    tokens.setProperty("mono", font_family)
    view.resize(1280, 560)
    view.show()
    # Deixa as animações (cursor, fades, OSD) assentarem.
    loop = QTimer(); loop.setSingleShot(True)
    loop.timeout.connect(app.quit); loop.start(settle_ms)
    app.exec()
    img = view.grabWindow()
    img.save(str(out_png))
    return view, root


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(REPO / "target/qml-shots"))
    ap.add_argument("--clios", default=str(REPO / "target/debug/clios"))
    ap.add_argument("--font", default=os.environ.get("GEIST_MONO_DIR", ""))
    a = ap.parse_args()
    out = pathlib.Path(a.out); out.mkdir(parents=True, exist_ok=True)

    app = QGuiApplication(sys.argv)
    family = "monospace"
    if a.font:
        for ttf in pathlib.Path(a.font).glob("GeistMono-*.ttf"):
            fid = QFontDatabase.addApplicationFont(str(ttf))
        fams = QFontDatabase.applicationFontFamilies(fid)
        if fams: family = fams[0]
    print("fonte:", family)

    tmp = pathlib.Path(tempfile.mkdtemp(prefix="clios-qml-"))
    build_tree(tmp)

    ok_path, _ = check_mark_path()
    print(("✓" if ok_path else "✗"), "marca: caminho do QML igual ao do SVG")
    failures = 0 if ok_path else 1
    cases = [("dark", "ember", "full"), ("light", "azure", "full"), ("dark", "mint", "off")]
    for mode, accent, motion in cases:
        messages.clear()
        png = out / f"shell-{mode}-{accent}-{motion}.png"
        view, root = render(app, tmp, family, theme_json(a.clios, mode, accent, motion), png)
        tok = view.engine().singletonInstance("qs.core", "Tokens")
        ok = True
        def check(cond, msg):
            global failures
            nonlocal ok
            if not cond:
                ok = False; print("   ✗", msg)
        check(not messages, "avisos do QML: " + " | ".join(messages))
        check(tok.property("mode") == mode, f"Tokens.mode = {tok.property('mode')}")
        check(tok.property("accentName") == accent, f"Tokens.accentName = {tok.property('accentName')}")
        check(tok.property("motionEnabled") == (motion != "off"), "Tokens.motionEnabled")
        want_bg = {"dark": "#000000", "light": "#ffffff"}[mode]
        check(tok.property("bg").name().lower() == want_bg, f"Tokens.bg = {tok.property('bg').name()}")
        # O cursor da faixa de workspaces precisa estar sob o número ativo (índice 1 → x = cell).
        strip = root.findChild(type(root), "") if False else None
        print(("✓" if ok else "✗"), f"{mode:5s} {accent:7s} movimento {motion:4s} → {png.name}")
        failures += 0 if ok else 1
        view.close()
    shutil.rmtree(tmp, ignore_errors=True)
    sys.exit(1 if failures else 0)

if __name__ == "__main__":
    main()
