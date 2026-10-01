#!/usr/bin/env python3
"""Confere se nomes de pacote existem no Arch, sem precisar do pacman.

Fontes (todas por `git ls-remote`, que atravessa proxies que bloqueiam archlinux.org):
  AUR        github.com/archlinux/aur            (uma branch por pacote do AUR)
  oficial    github.com/archlinux/svntogit-packages e svntogit-community
             (arquivados em 2023: confirmam o que existia até lá; pacotes mais novos podem faltar)

uso: pkgcheck.py nome [nome ...]      ou      pkgcheck.py -f arquivo.txt
Saída: uma linha por pacote com as fontes onde foi achado.
"""
import subprocess, sys

def ls_remote(url, patterns):
    found = set()
    for i in range(0, len(patterns), 40):
        chunk = patterns[i:i + 40]
        out = subprocess.run(["git", "ls-remote", "--heads", url, *chunk], capture_output=True, text=True, timeout=180).stdout
        for line in out.splitlines():
            found.add(line.split("\t")[1].removeprefix("refs/heads/"))
    return found

def check(names):
    aur = ls_remote("https://github.com/archlinux/aur.git", [f"refs/heads/{n}" for n in names])
    pk = ls_remote("https://github.com/archlinux/svntogit-packages.git", [f"refs/heads/packages/{n}" for n in names])
    cm = ls_remote("https://github.com/archlinux/svntogit-community.git", [f"refs/heads/packages/{n}" for n in names])
    res = {}
    for n in names:
        src = []
        if f"packages/{n}" in pk: src.append("core/extra")
        if f"packages/{n}" in cm: src.append("community")
        if n in aur: src.append("AUR")
        res[n] = src
    return res

if __name__ == "__main__":
    args = sys.argv[1:]
    if args[:1] == ["-f"]:
        names = [l.split("#")[0].strip() for l in open(args[1]) if l.split("#")[0].strip()]
    else:
        names = args
    for n, src in check(names).items():
        print(f"{n:22s} {', '.join(src) if src else '— não achei'}")
