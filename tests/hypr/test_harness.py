#!/usr/bin/env python3
"""O validador também precisa ser testado: configs ruins conhecidas têm que ser reprovadas."""
import pathlib, sys, tempfile
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import check_config as cc

stub = cc.load_stub()

BAD = {
    "opção de config inexistente": ('hl.config({ general = { gaps_inn = 1 } })', "gaps_inn"),
    "tipo errado": ('hl.config({ general = { border_size = "grande" } })', "border_size"),
    "dispatcher inexistente": ('hl.bind("SUPER + X", hl.dsp.window.closee(), { description = "x" })', "closee"),
    "função da API inexistente": ('hl.bindd("SUPER + X", hl.dsp.window.close())', "bindd"),
    "folha de animação inexistente": ('hl.curve("c", {type="bezier", points={{0,0},{1,1}}}); hl.animation({leaf="janelas", enabled=true, speed=1, bezier="c"})', "janelas"),
    "estilo inválido": ('hl.curve("c", {type="bezier", points={{0,0},{1,1}}}); hl.animation({leaf="windows", enabled=true, speed=1, bezier="c", style="slidee"})', "slidee"),
    "curva não registrada": ('hl.animation({leaf="windows", enabled=true, speed=1, bezier="fantasma"})', "fantasma"),
    "bezier com x fora de [0,1]": ('hl.curve("c", {type="bezier", points={{1.5,0},{1,1}}})', "fora de [0,1]"),
    "bezier e spring juntos": ('hl.curve("a", {type="bezier", points={{0,0},{1,1}}}); hl.curve("b", {type="spring", mass=1, stiffness=1, damping=1}); hl.animation({leaf="windows", enabled=true, speed=1, bezier="a", spring="b"})', "exatamente um"),
    "atalho duplicado": ('hl.bind("SUPER + X", hl.dsp.window.close()); hl.bind("SUPER + X", hl.dsp.window.close())', "duplicado"),
    "modificador inválido": ('hl.bind("WIN + X", hl.dsp.window.close())', "WIN"),
    "efeito de regra inexistente": ('hl.window_rule({ match = { class = "x" }, flutuar = true })', "flutuar"),
    "match inexistente": ('hl.window_rule({ match = { classe = "x" }, float = true })', "classe"),
    "regra sem efeito": ('hl.window_rule({ name = "vazia", match = { class = "x" } })', "sem nenhum efeito"),
    "evento inexistente": ('hl.on("window.abriu", function() end)', "window.abriu"),
    "timer sem tipo": ('hl.timer(function() end, { timeout = 10 })', "timer"),
    "erro de runtime": ('local x = nil; x.y = 1', "falhou"),
    "campo de monitor inexistente": ('hl.monitor({ output = "", resolucao = "x" })', "resolucao"),
}

OK = 'hl.config({ general = { gaps_in = 5, border_size = 2 } })\nhl.bind("SUPER + X", hl.dsp.window.close(), { description = "fechar" })\n'

def run(src):
    with tempfile.TemporaryDirectory() as d:
        p = pathlib.Path(d)
        (p / "theme.lua").write_text("return {}")
        (p / "hyprland.lua").write_text(src)
        return cc.run_once(stub, p)[0]

fails = 0
errs = run(OK)
if errs:
    print("✗ a config boa foi reprovada:", errs); fails += 1
else:
    print("✓ config boa passa")
for name, (src, needle) in BAD.items():
    errs = run(OK + src)
    hit = any(needle in e for e in errs)
    print(("✓" if hit else "✗"), f"reprova: {name}")
    if not hit:
        fails += 1
        print("    erros recebidos:", errs)
sys.exit(1 if fails else 0)
