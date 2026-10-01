#!/usr/bin/env python3
"""Roda um programa de terminal de verdade num pty, deixa ele desenhar por alguns segundos e grava a tela
como ANSI de 24 bits (o formato que o `ansi2png.py` lê). Serve para as imagens da documentação e do site:
o fastfetch, o bonsai, a chuva digital e o relógio aparecem como aparecem no foot, com a paleta do CLIOS.

uso: termshot.py --colors ~/.config/foot/colors.ini --cols 100 --rows 28 --seconds 3 --out x.ansi -- cbonsai -li
Requer: pip install pyte
"""
import argparse, fcntl, os, pty, re, select, signal, struct, sys, termios, time
import pyte

NAMES = ["black", "red", "green", "brown", "blue", "magenta", "cyan", "white"]  # nomes do pyte (brown = yellow)


def read_palette(path):
    """O colors.ini que o `clios theme apply` gera: foreground, background e as 16 cores."""
    p = {}
    for line in open(path):
        m = re.match(r"\s*(\w+)\s*=\s*([0-9a-fA-F]{6})", line)
        if m:
            p[m.group(1)] = tuple(int(m.group(2)[i:i + 2], 16) for i in (0, 2, 4))
    return p


def run(cmd, cols, rows, seconds, env_extra):
    screen = pyte.Screen(cols, rows)
    stream = pyte.ByteStream(screen)
    pid, fd = pty.fork()
    if pid == 0:
        env = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor", COLUMNS=str(cols), LINES=str(rows), **env_extra)
        os.execvpe(cmd[0], cmd, env)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    os.kill(pid, signal.SIGWINCH)
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            stream.feed(data)
    try:
        os.kill(pid, signal.SIGTERM)
        os.waitpid(pid, 0)
    except OSError:
        pass
    return screen


def color(name, palette, default, bold=False):
    if name == "default":
        return default
    if name.startswith("bright") and name[6:] in NAMES:
        return palette.get("bright%d" % NAMES.index(name[6:]), default)
    if name in NAMES:
        i = NAMES.index(name) + (8 if bold else 0)
        return palette.get(("bright%d" % (i - 8)) if i >= 8 else ("regular%d" % i), default)
    if re.fullmatch(r"[0-9a-fA-F]{6}", name):
        return tuple(int(name[i:i + 2], 16) for i in (0, 2, 4))
    return default


def to_ansi(screen, palette):
    fg0, bg0 = palette["foreground"], palette["background"]
    out = []
    for y in range(screen.lines):
        line, last = "", None
        row = screen.buffer[y]
        for x in range(screen.columns):
            c = row[x]
            fg = color(c.fg, palette, fg0, c.bold)
            bg = color(c.bg, palette, bg0)
            if c.reverse:
                fg, bg = bg, fg
            key = (fg, bg, c.bold)
            if key != last:
                line += "\x1b[0;%s38;2;%d;%d;%d;48;2;%d;%d;%dm" % ("1;" if c.bold else "", *fg, *bg)
                last = key
            line += c.data or " "
        out.append(line + "\x1b[0m")
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--colors", required=True)
    ap.add_argument("--cols", type=int, default=100)
    ap.add_argument("--rows", type=int, default=28)
    ap.add_argument("--seconds", type=float, default=3.0)
    ap.add_argument("--out", required=True)
    ap.add_argument("--env", action="append", default=[], help="CHAVE=valor")
    ap.add_argument("cmd", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    cmd = a.cmd[1:] if a.cmd[:1] == ["--"] else a.cmd
    if not cmd:
        sys.exit("falta o comando depois de --")
    pal = read_palette(a.colors)
    scr = run(cmd, a.cols, a.rows, a.seconds, dict(e.split("=", 1) for e in a.env))
    open(a.out, "w").write(to_ansi(scr, pal))
