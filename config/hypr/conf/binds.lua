-- Atalhos. Tudo tem `description`: é de lá que o hub (`SUPER + ?`) tira a cola de atalhos,
-- então a documentação nunca fica velha.
--
-- Filosofia: SUPER faz o desktop; as mãos ficam no home row (h j k l).
--   SUPER + tecla            ir
--   SUPER + SHIFT + tecla    mover
--   SUPER + CTRL + tecla     redimensionar / variação
return function(_)
  local M = "SUPER"

  local function bind(keys, dsp, desc, opts)
    opts = opts or {}
    opts.description = desc
    return hl.bind(keys, dsp, opts)
  end
  local function run(cmd) return hl.dsp.exec_cmd(cmd) end
  local function open(id) return run("clios open " .. id) end

  -- ── o hub: tudo começa aqui ──────────────────────────────────────────────
  bind(M .. " + space", open("hub"), "abrir o hub")
  bind(M .. " + Escape", run("clios open hub '>'"), "sessão: bloquear, suspender, sair")
  bind(M .. " + V", run("clios open hub '\"'"), "histórico da área de transferência")
  bind(M .. " + slash", run("clios open hub '?'"), "ver todos os atalhos")
  bind(M .. " + Tab", hl.dsp.focus({ workspace = "previous" }), "alternar para a workspace anterior")

  -- ── terminal ─────────────────────────────────────────────────────────────
  -- `clios term` abre o footclient com uma ficha: é o que permite reabrir a sessão e abrir na mesma pasta.
  bind(M .. " + Return", run("clios term"), "terminal")
  bind(M .. " + SHIFT + Return", run("clios term --float"), "terminal flutuante")
  bind(M .. " + CTRL + Return", run("clios term --here"), "terminal na mesma pasta do que está em foco")
  bind(M .. " + S", hl.dsp.workspace.toggle_special("scratch"), "scratchpad: mostrar ou esconder")
  bind(M .. " + CTRL + S", hl.dsp.window.move({ workspace = "special:scratch" }), "enviar janela ao scratchpad")

  -- ── apps de terminal (ids em config/clios/hub.toml) ──────────────────────
  bind(M .. " + E", open("files"), "arquivos")
  bind(M .. " + G", open("git"), "git")
  bind(M .. " + B", open("monitor"), "monitor do sistema")
  bind(M .. " + A", open("audio"), "áudio")
  bind(M .. " + I", open("network"), "rede (wi-fi)")
  bind(M .. " + SHIFT + B", open("bluetooth"), "bluetooth")
  bind(M .. " + M", open("music"), "música")
  bind(M .. " + W", run("firefox"), "navegador (a única exceção gráfica)")

  -- ── janelas ──────────────────────────────────────────────────────────────
  bind(M .. " + Q", hl.dsp.window.close(), "fechar janela")
  bind(M .. " + F", hl.dsp.window.fullscreen({ mode = "fullscreen" }), "tela cheia")
  bind(M .. " + SHIFT + F", hl.dsp.window.fullscreen({ mode = "maximized" }), "maximizar (mantém a barra)")
  bind(M .. " + T", hl.dsp.window.float({ action = "toggle" }), "alternar flutuante")
  bind(M .. " + comma", hl.dsp.layout("togglesplit"), "trocar direção da divisão")
  bind(M .. " + period", hl.dsp.window.pin(), "fixar em todas as workspaces")

  local dirs = { h = "left", j = "down", k = "up", l = "right" }
  local arrows = { left = "left", down = "down", up = "up", right = "right" }
  for key, dir in pairs(dirs) do
    bind(M .. " + " .. key, hl.dsp.focus({ direction = dir }), "foco: " .. dir)
    bind(M .. " + SHIFT + " .. key, hl.dsp.window.move({ direction = dir }), "mover janela: " .. dir)
  end
  for key, dir in pairs(arrows) do
    bind(M .. " + " .. key, hl.dsp.focus({ direction = dir }), "foco: " .. dir)
    bind(M .. " + SHIFT + " .. key, hl.dsp.window.move({ direction = dir }), "mover janela: " .. dir)
  end

  -- monitores: O de "outro". Com um monitor só, não fazem nada.
  bind(M .. " + O", hl.dsp.focus({ monitor = "+1" }), "foco no outro monitor")
  bind(M .. " + SHIFT + O", hl.dsp.window.move({ monitor = "+1" }), "mandar a janela para o outro monitor")
  bind(M .. " + CTRL + O", hl.dsp.workspace.move({ monitor = "+1" }), "mandar a workspace para o outro monitor")

  -- redimensionar: modo `resize` (SUPER + R), h j k l, Esc sai
  bind(M .. " + R", hl.dsp.submap("resize"), "modo redimensionar (h j k l, esc sai)")
  hl.define_submap("resize", function()
    local step = 40
    local function rs(x, y) return hl.dsp.window.resize({ x = x, y = y, relative = true }) end
    hl.bind("h", rs(-step, 0), { repeating = true, description = "redimensionar: esquerda" })
    hl.bind("l", rs(step, 0), { repeating = true, description = "redimensionar: direita" })
    hl.bind("k", rs(0, -step), { repeating = true, description = "redimensionar: cima" })
    hl.bind("j", rs(0, step), { repeating = true, description = "redimensionar: baixo" })
    hl.bind("escape", hl.dsp.submap("reset"), { description = "sair do modo redimensionar" })
    hl.bind("Return", hl.dsp.submap("reset"), { description = "sair do modo redimensionar" })
  end)

  -- ── workspaces ───────────────────────────────────────────────────────────
  for i = 1, 10 do
    local key = i % 10 -- a workspace 10 fica na tecla 0
    bind(M .. " + " .. key, hl.dsp.focus({ workspace = i }), "ir para a workspace " .. i)
    bind(M .. " + SHIFT + " .. key, hl.dsp.window.move({ workspace = i }), "enviar janela para a workspace " .. i)
  end
  bind(M .. " + bracketleft", hl.dsp.focus({ workspace = "e-1" }), "workspace anterior")
  bind(M .. " + bracketright", hl.dsp.focus({ workspace = "e+1" }), "próxima workspace")

  -- ── mouse ────────────────────────────────────────────────────────────────
  bind(M .. " + mouse:272", hl.dsp.window.drag(), "mover janela arrastando", { mouse = true })
  bind(M .. " + mouse:273", hl.dsp.window.resize(), "redimensionar arrastando", { mouse = true })
  bind(M .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }), "próxima workspace (scroll)")
  bind(M .. " + mouse_up", hl.dsp.focus({ workspace = "e-1" }), "workspace anterior (scroll)")

  -- ── tema e movimento ─────────────────────────────────────────────────────
  bind(M .. " + F1", run("clios theme toggle"), "tema: claro ou escuro")
  bind(M .. " + F2", run("clios theme cycle"), "tema: próximo acento")
  bind(M .. " + F3", run("clios motion"), "movimento: completo, reduzido, desligado")
  bind(M .. " + F4", open("wallpaper"), "papel de parede: escolher")
  bind(M .. " + SHIFT + F4", run("clios wallpaper next"), "papel de parede: próximo")
  bind(M .. " + F9", open("central"), "central do sistema")
  bind(M .. " + F10", open("welcome"), "guia de boas-vindas")

  -- ── ferramentas do dia a dia ─────────────────────────────────────────────
  bind(M .. " + C", run("clios caffeine"), "modo café: a tela não apaga")
  bind(M .. " + N", run("clios night"), "modo noturno: tela mais quente")
  bind(M .. " + D", run("clios dnd"), "não perturbe: silenciar notificações")
  bind(M .. " + SHIFT + N", open("notifs"), "histórico de notificações")
  bind(M .. " + X", run("clios focus"), "foco: 25 minutos em silêncio (de novo, para)")
  bind(M .. " + SHIFT + R", run("clios rec"), "gravar a tela: começar ou parar")
  bind(M .. " + P", run("clios pick"), "conta-gotas: copiar uma cor da tela")
  bind(M .. " + SHIFT + T", run("clios ocr"), "texto da tela: selecionar e copiar (OCR)")
  bind(M .. " + U", open("update"), "atualizar o sistema")
  bind(M .. " + Z", open("play"), "brincar: um brinquedo para espairecer")

  -- ── capturas ─────────────────────────────────────────────────────────────
  bind("Print", run("clios shot region"), "captura: região")
  bind(M .. " + SHIFT + S", run("clios shot region"), "captura: região (o mesmo que Print)")
  bind("SHIFT + Print", run("clios shot screen"), "captura: tela inteira")
  bind("CTRL + Print", run("clios shot window"), "captura: janela")

  -- ── sessão ───────────────────────────────────────────────────────────────
  bind(M .. " + CTRL + Q", run("pidof hyprlock || hyprlock"), "bloquear a tela")

  -- ── teclas de mídia (funcionam com a tela bloqueada) ─────────────────────
  local locked = { locked = true, repeating = true }
  local function media(keys, cmd, desc, extra)
    local o = { locked = true }
    for k, v in pairs(extra or {}) do o[k] = v end
    bind(keys, run(cmd), desc, o)
  end
  media("XF86AudioRaiseVolume", "wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+", "volume +", locked)
  media("XF86AudioLowerVolume", "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-", "volume -", locked)
  media("XF86AudioMute", "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle", "silenciar")
  media("XF86AudioMicMute", "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle", "silenciar microfone")
  -- O OSD de volume aparece sozinho (a shell observa o PipeWire). O de brilho é avisado.
  media("XF86MonBrightnessUp", "brightnessctl -e4 -n2 set 5%+ && qs ipc call osd brightness", "brilho +", locked)
  media("XF86MonBrightnessDown", "brightnessctl -e4 -n2 set 5%- && qs ipc call osd brightness", "brilho -", locked)
  media("XF86AudioPlay", "playerctl play-pause", "tocar ou pausar")
  media("XF86AudioPause", "playerctl play-pause", "tocar ou pausar")
  media("XF86AudioNext", "playerctl next", "próxima faixa")
  media("XF86AudioPrev", "playerctl previous", "faixa anterior")
end
