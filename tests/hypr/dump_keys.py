#!/usr/bin/env python3
"""Gera docs/KEYS.md a partir do próprio config Lua (a documentação não diverge do que roda)."""
import pathlib, subprocess, sys, tempfile
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import check_config as cc

repo = pathlib.Path(__file__).resolve().parents[2]
clios = str(repo / "target/debug/clios")
with tempfile.TemporaryDirectory() as home:
    base = [clios, "--root", str(repo), "--home", home]
    subprocess.run(base + ["sync", "--copy"], check=True, capture_output=True)
    _, binds, _, _, _ = cc.run_once(cc.load_stub(), pathlib.Path(home) / ".config/hypr")

def pretty(keys):
    return keys.replace("SUPER", "super").replace("SHIFT", "shift").replace("CTRL", "ctrl").replace(" + ", " + ")

groups = {}
for keys, desc, submap in binds:
    groups.setdefault(submap or "", []).append((keys, desc))

out = ["# Atalhos", "",
       "Gerado de `config/hypr/conf/binds.lua` por `tests/hypr/dump_keys.py`. No sistema, `SUPER + /` mostra esta mesma lista no hub.", ""]
for submap, rows in groups.items():
    out.append(f"## {'modo ' + submap if submap else 'globais'}")
    out += ["", "| atalho | ação |", "|---|---|"]
    for keys, desc in rows:
        out.append(f"| `{pretty(keys)}` | {desc} |")
    out.append("")
(repo / "docs/KEYS.md").write_text("\n".join(out))
print("docs/KEYS.md:", len(binds), "atalhos")
