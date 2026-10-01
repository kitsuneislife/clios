#!/usr/bin/env python3
"""Pega a config do fastfetch que o `clios theme apply` gerou e troca os valores detectados por valores de exemplo,
mantendo tudo o mais (chaves, cores, logo, larguras). O fastfetch desenha de verdade; só os números são de mentira,
para a imagem do site não mostrar a máquina de quem a gerou.

uso: fetch_demo.py config.jsonc saida.jsonc
"""
import json, re, sys

DEMO = {
    "os": "Arch Linux", "kernel": "6.18.3-arch1-1", "uptime": "2 min", "packages": "612", "shell": "fish",
    "wm": "Hyprland", "terminal": "foot", "cpu": "AMD Ryzen 7 7840U", "gpu": "AMD Radeon 780M",
    "memory": "1.4 GiB / 32 GiB", "disk": "182 GiB / 953 GiB", "battery": "87% Discharging",
}

src = re.sub(r"(?m)^\s*//.*$", "", open(sys.argv[1]).read())
cfg = json.loads(src)
mods = []
for m in cfg["modules"]:
    if m == "title":
        mods.append({"type": "custom", "format": "você@clios", "outputColor": cfg["display"]["color"]["title"]})
    elif isinstance(m, dict) and m.get("type") in DEMO:
        mods.append({"type": "custom", "key": m.get("key", m["type"]), "format": DEMO[m["type"]]})
    else:
        mods.append(m)
cfg["modules"] = mods
json.dump(cfg, open(sys.argv[2], "w"), ensure_ascii=False, indent=1)
